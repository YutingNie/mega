use std::{
    collections::{HashMap, HashSet},
    path::{Component, PathBuf},
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
    },
    vec,
};

use async_trait::async_trait;
use bytes::Bytes;

use callisto::{mega_tree, raw_blob};
use common::{errors::MegaError, utils::MEGA_BRANCH_NAME};
use jupiter::{context::Context, storage::batch_query_by_columns};
use mercury::internal::pack::encode::PackEncoder;
use venus::{
    errors::GitError,
    hash::SHA1,
    internal::{
        object::{blob::Blob, commit::Commit, tag::Tag, tree::Tree, types::ObjectType},
        pack::{
            entry::Entry,
            reference::{RefCommand, Refs},
        },
    },
    monorepo::mr::MergeRequest,
    repo::Repo,
};

use crate::pack::handler::PackHandler;

pub struct MonoRepo {
    pub context: Context,
    pub path: PathBuf,
    pub from_hash: Option<String>,
    pub to_hash: Option<String>,
}

#[async_trait]
impl PackHandler for MonoRepo {
    async fn head_hash(&self) -> (String, Vec<Refs>) {
        let storage = self.context.services.mega_storage.clone();

        let result = storage.get_ref(self.path.to_str().unwrap()).await.unwrap();
        let refs = if result.is_some() {
            vec![result.unwrap().into()]
        } else {
            let target_path = self.path.clone();
            let ref_hash = storage
                .get_ref("/")
                .await
                .unwrap()
                .unwrap()
                .ref_commit_hash
                .clone();

            let commit: Commit = storage
                .get_commit_by_hash(&Repo::empty(), &ref_hash)
                .await
                .unwrap()
                .unwrap()
                .into();
            let tree_id = commit.tree_id.to_plain_str();
            let mut tree: Tree = storage
                .get_trees_by_hashes(&Repo::empty(), vec![tree_id])
                .await
                .unwrap()[0]
                .clone()
                .into();

            for component in target_path.components() {
                if component != Component::RootDir {
                    let path_name = component.as_os_str().to_str().unwrap();
                    let sha1 = tree
                        .tree_items
                        .iter()
                        .find(|x| x.name == path_name)
                        .map(|x| x.id);
                    if let Some(sha1) = sha1 {
                        tree = storage
                            .get_trees_by_hashes(&Repo::empty(), vec![sha1.to_plain_str()])
                            .await
                            .unwrap()[0]
                            .clone()
                            .into();
                    } else {
                        return self.check_head_hash(vec![]);
                    }
                }
            }

            let c = Commit::from_tree_id(
                tree.id,
                vec![],
                "This commit was generated by mega for maintain refs",
            );
            storage
                .save_ref(
                    self.path.to_str().unwrap(),
                    &c.id.to_plain_str(),
                    &c.tree_id.to_plain_str(),
                )
                .await
                .unwrap();
            storage
                .save_mega_commits(&Repo::empty(), vec![c.clone()])
                .await
                .unwrap();

            vec![Refs {
                ref_name: MEGA_BRANCH_NAME.to_string(),
                ref_hash: c.id.to_plain_str(),
                default_branch: true,
                ..Default::default()
            }]
        };
        self.check_head_hash(refs)
    }

    async fn unpack(&self, pack_file: Bytes) -> Result<(), GitError> {
        let receiver = self.pack_decoder(pack_file).unwrap();

        let storage = self.context.services.mega_storage.clone();

        let (mut mr, mr_exist) = self.get_mr().await;

        let mut commit_size = 0;
        if mr_exist {
            if mr.from_hash == self.from_hash.clone().unwrap() {
                let to_hash = self.to_hash.clone().unwrap();
                if mr.to_hash != to_hash {
                    let comment = self.comment_for_force_update(&mr.to_hash, &to_hash);
                    mr.to_hash = to_hash;
                    storage
                        .add_mr_comment(mr.id, 0, Some(comment))
                        .await
                        .unwrap();
                    commit_size = self.save_entry(receiver).await;
                }
            } else {
                mr.close();
                storage
                    .add_mr_comment(mr.id, 0, Some("Mega closed MR due to conflict".to_string()))
                    .await
                    .unwrap();
            }
            storage.update_mr(mr.clone()).await.unwrap();
        } else {
            commit_size = self.save_entry(receiver).await;

            storage.save_mr(mr.clone()).await.unwrap();
        };

        if commit_size > 1 {
            mr.close();
            storage
                .add_mr_comment(
                    mr.id,
                    0,
                    Some("Mega closed MR due to multi commit detected".to_string()),
                )
                .await
                .unwrap();
        }
        Ok(())
    }

    async fn full_pack(&self) -> Result<Vec<u8>, GitError> {
        let (sender, receiver) = mpsc::channel();
        let repo = &Repo::empty();
        let storage = self.context.services.mega_storage.clone();
        // let obj_num = storage.get_obj_count_by_repo_id(repo).await;
        let mut obj_num = 0;
        let mut encoder = PackEncoder::new(obj_num, 0);

        for m in storage
            .get_commits_by_repo_id(repo)
            .await
            .unwrap()
            .into_iter()
        {
            let c: Commit = m.into();
            let entry: Entry = c.into();
            sender.send(entry).unwrap();
            obj_num += 1;
        }

        for m in storage
            .get_trees_by_repo_id(repo)
            .await
            .unwrap()
            .into_iter()
        {
            let c: Tree = m.into();
            let entry: Entry = c.into();
            sender.send(entry).unwrap();
            obj_num += 1;
        }

        let bids: Vec<String> = storage
            .get_blobs_by_repo_id(repo)
            .await
            .unwrap()
            .into_iter()
            .map(|b| b.blob_id)
            .collect();

        let raw_blobs = batch_query_by_columns::<raw_blob::Entity, raw_blob::Column>(
            storage.get_connection(),
            raw_blob::Column::Sha1,
            bids,
            None,
            None,
        )
        .await
        .unwrap();

        for m in raw_blobs {
            // todo handle storage type
            let c: Blob = m.into();
            let entry: Entry = c.into();
            sender.send(entry).unwrap();
            obj_num += 1;
        }

        for m in storage.get_tags_by_repo_id(repo).await.unwrap().into_iter() {
            let c: Tag = m.into();
            let entry: Entry = c.into();
            sender.send(entry).unwrap();
            obj_num += 1;
        }
        drop(sender);
        let data = encoder.encode(receiver).unwrap();

        Ok(data)
    }

    async fn incremental_pack(
        &self,
        want: Vec<String>,
        have: Vec<String>,
    ) -> Result<Vec<u8>, GitError> {
        let (sender, receiver) = mpsc::channel();
        let repo = Repo::empty();
        let storage = self.context.services.mega_storage.clone();
        // let obj_num = storage.get_obj_count_by_repo_id(repo).await;
        let obj_num = AtomicUsize::new(0);

        let mut exist_objs = HashSet::new();

        let commits: Vec<Commit> = storage
            .get_commits_by_hashes(&repo, want)
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.into())
            .collect();
        let mut traversal_list: Vec<Commit> = commits.clone();
        let mut want_commits: Vec<Commit> = commits;

        // tarverse commit's all parents to find the commit that client does not have
        while let Some(temp) = traversal_list.pop() {
            for p_commit_id in temp.parent_commit_ids {
                let p_commit_id = &p_commit_id.to_plain_str();

                let want_commit_ids: Vec<String> =
                    want_commits.iter().map(|x| x.id.to_plain_str()).collect();

                if !have.contains(p_commit_id) && !want_commit_ids.contains(p_commit_id) {
                    let parent: Commit = storage
                        .get_commit_by_hash(&repo, p_commit_id)
                        .await
                        .unwrap()
                        .unwrap()
                        .into();
                    want_commits.push(parent.clone());
                    traversal_list.push(parent);
                }
            }
        }

        let want_tree_ids = want_commits
            .iter()
            .map(|c| c.tree_id.to_plain_str())
            .collect();
        let want_trees: HashMap<SHA1, Tree> = storage
            .get_trees_by_hashes(&repo, want_tree_ids)
            .await
            .unwrap()
            .into_iter()
            .map(|m| (SHA1::from_str(&m.tree_id).unwrap(), m.into()))
            .collect();

        for c in want_commits {
            let have_commit_hashes: Vec<String> = c
                .parent_commit_ids
                .clone()
                .into_iter()
                .filter(|p_id| have.contains(&p_id.to_plain_str()))
                .map(|hash| hash.to_plain_str())
                .collect();
            let have_commits = storage
                .get_commits_by_hashes(&repo, have_commit_hashes)
                .await
                .unwrap();

            for have_c in have_commits {
                let have_tree = storage
                    .get_trees_by_hashes(&repo, vec![have_c.tree])
                    .await
                    .unwrap()[0]
                    .clone()
                    .into();
                self.add_to_exist_objs(have_tree, &mut exist_objs).await;
            }

            self.traverse_want_trees(
                want_trees.get(&c.tree_id).unwrap().clone(),
                &exist_objs,
                sender.clone(),
                &obj_num,
            )
            .await;
            sender.send(c.into()).unwrap();
            obj_num.fetch_add(1, Ordering::SeqCst);
        }
        drop(sender);
        let mut encoder = PackEncoder::new(obj_num.into_inner(), 0);
        let data = encoder.encode(receiver).unwrap();
        Ok(data)
    }

    async fn get_trees_by_hashes(
        &self,
        hashes: Vec<String>,
    ) -> Result<Vec<mega_tree::Model>, MegaError> {
        self.context
            .services
            .mega_storage
            .get_trees_by_hashes(&Repo::empty(), hashes)
            .await
    }

    async fn get_blobs_by_hashes(
        &self,
        hashes: Vec<String>,
    ) -> Result<Vec<raw_blob::Model>, MegaError> {
        self.context
            .services
            .mega_storage
            .get_raw_blobs_by_hashes(hashes)
            .await
    }

    async fn update_refs(&self, _: &RefCommand) -> Result<(), GitError> {
        //do nothing in monorepo because need mr to handle refs
        Ok(())
    }

    async fn check_commit_exist(&self, hash: &str) -> bool {
        self.context
            .services
            .mega_storage
            .get_commit_by_hash(&Repo::empty(), hash)
            .await
            .unwrap()
            .is_some()
    }

    async fn check_default_branch(&self) -> bool {
        true
    }
}

impl MonoRepo {
    async fn get_mr(&self) -> (MergeRequest, bool) {
        let storage = self.context.services.mega_storage.clone();

        let mr = storage
            .get_open_mr(self.path.to_str().unwrap())
            .await
            .unwrap();
        if let Some(mr) = mr {
            (mr, true)
        } else {
            let mr = MergeRequest {
                path: self.path.to_str().unwrap().to_owned(),
                from_hash: self.from_hash.clone().unwrap(),
                to_hash: self.to_hash.clone().unwrap(),
                ..Default::default()
            };
            (mr, false)
        }
    }

    fn comment_for_force_update(&self, from: &str, to: &str) -> String {
        format!(
            "Mega updated the mr automatic from {} to {}",
            &from[..6],
            &to[..6]
        )
    }

    async fn save_entry(&self, receiver: Receiver<Entry>) -> i32 {
        let storage = self.context.services.mega_storage.clone();
        let mut entry_list = Vec::new();

        let mut commit_size = 0;
        for entry in receiver {
            if entry.obj_type == ObjectType::Commit {
                commit_size += 1;
            }
            entry_list.push(entry);
            if entry_list.len() >= 1000 {
                storage.save_entry(entry_list).await.unwrap();
                entry_list = Vec::new();
            }
        }
        storage.save_entry(entry_list).await.unwrap();
        commit_size
    }
}
