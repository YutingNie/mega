//! In Git, a tree object is used to represent the state of a directory at a specific point in time.
//! It stores information about the files and directories within that directory, including their names,
//! permissions, and the IDs of the objects that represent their contents.
//!
//! A tree object can contain other tree objects as well as blob objects, which represent the contents
//! of individual files. The object IDs of these child objects are stored within the tree object itself.
//!
//! When you make a commit in Git, you create a new tree object that represents the state of the
//! repository at that point in time. The parent of the new commit is typically the tree object
//! representing the previous state of the repository.
//!
//! Git uses the tree object to efficiently store and manage the contents of a repository. By
//! representing the contents of a directory as a tree object, Git can quickly determine which files
//! have been added, modified, or deleted between two points in time. This allows Git to perform
//! operations like merging and rebasing more quickly and accurately.
//!
use std::fmt::Display;

use colored::Colorize;
use bstr::ByteSlice;

use crate::git::errors::GitError;
use crate::git::hash::Hash;
use crate::git::internal::object::meta::Meta;

/// In Git, the mode field in a tree object's entry specifies the type of the object represented by
/// that entry. The mode is a three-digit octal number that encodes both the permissions and the
/// type of the object. The first digit specifies the object type, and the remaining two digits
/// specify the file mode or permissions.
#[allow(unused)]
#[derive(PartialEq, Eq, Hash, Ord, PartialOrd, Debug, Clone, Copy)]
pub enum TreeItemMode {
    Blob,
    BlobExecutable,
    Tree,
    Commit,
    Link,
}

impl Display for TreeItemMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let _print = match *self {
            TreeItemMode::Blob => "blob",
            TreeItemMode::BlobExecutable => "blob executable",
            TreeItemMode::Tree => "tree",
            TreeItemMode::Commit => "commit",
            TreeItemMode::Link => "link",
        };

        write!(f, "{}", String::from(_print).blue())
    }
}

impl TreeItemMode {

    /// 32-bit mode, split into (high to low bits):
    /// - 4-bit object type: valid values in binary are 1000 (regular file), 1010 (symbolic link) and 1110 (gitlink)
    /// - 3-bit unused
    /// - 9-bit unix permission: Only 0755 and 0644 are valid for regular files. Symbolic links and gitlink have value 0 in this field.
    #[allow(unused)]
    pub fn to_bytes(self) -> &'static [u8] {
        match self {
            TreeItemMode::Blob => b"100644",
            TreeItemMode::BlobExecutable => b"100755",
            TreeItemMode::Link => b"120000",
            TreeItemMode::Tree => b"40000",
            TreeItemMode::Commit => b"160000",
        }
    }

    /// Convert a 32-bit mode to a TreeItemType
    ///
    /// |0100000000000000| (040000)| Directory|
    /// |1000000110100100| (100644)| Regular non-executable file|
    /// |1000000110110100| (100664)| Regular non-executable group-writeable file|
    /// |1000000111101101| (100755)| Regular executable file|
    /// |1010000000000000| (120000)| Symbolic link|
    /// |1110000000000000| (160000)| Gitlink|
    /// ---
    /// # GitLink
    /// Gitlink, also known as a submodule, is a feature in Git that allows you to include a Git
    /// repository as a subdirectory within another Git repository. This is useful when you want to
    /// incorporate code from another project into your own project, without having to manually copy
    /// the code into your repository.
    ///
    /// When you add a submodule to your Git repository, Git stores a reference to the other
    /// repository at a specific commit. This means that your repository will always point to a
    /// specific version of the other repository, even if changes are made to the submodule's code
    /// in the future.
    ///
    /// To work with a submodule in Git, you use commands like git submodule add, git submodule
    /// update, and git submodule init. These commands allow you to add a submodule to your repository,
    /// update it to the latest version, and initialize it for use.
    ///
    /// Submodules can be a powerful tool for managing dependencies between different projects and
    /// components. However, they can also add complexity to your workflow, so it's important to
    /// understand how they work and when to use them.
    #[allow(unused)]
    pub fn tree_item_type_from(mode: &[u8]) -> Result<TreeItemMode, GitError> {
        Ok(match mode {
            b"40000" => TreeItemMode::Tree,
            b"100644" => TreeItemMode::Blob,
            b"100755" => TreeItemMode::BlobExecutable,
            b"120000" => TreeItemMode::Link,
            b"160000" => TreeItemMode::Commit,
            b"100664" => TreeItemMode::Blob,
            b"100640" => TreeItemMode::Blob,
            _ => {
                return Err(GitError::InvalidTreeItem(
                    String::from_utf8(mode.to_vec()).unwrap(),
                ));
            }
        })
    }
}

/// A tree object contains a list of entries, one for each file or directory in the tree. Each entry
/// in the file represents an entry in the tree, and each entry has the following format:
///
/// ```bash
/// <mode> <name>\0<binary object ID>
/// ```
/// - `<mode>` is the mode of the object, represented as a six-digit octal number. The first digit
/// represents the object type (tree, blob, etc.), and the remaining digits represent the file mode or permissions.
/// - `<name>` is the name of the object.
/// - `\0` is a null byte separator.
/// - `<binary object ID>` is the ID of the object that represents the contents of the file or
/// directory, represented as a binary SHA-1 hash.
///
/// # Example
/// ```bash
/// 100644 hello-world\0<blob object ID>
/// 040000 data\0<tree object ID>
/// ```
#[allow(unused)]
#[derive(PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Clone)]
pub struct TreeItem {
    pub mode: TreeItemMode,
    pub id: Hash,
    pub name: String,
}

impl Display for TreeItem {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.mode,
            self.name,
            self.id.to_string().blue()
        )
    }
}

impl TreeItem {
    /// Create a new TreeItem from a mode, id and name
    ///
    /// # Example
    /// ```rust
    /// // Create a empty TreeItem with the default Hash
    /// let default_item = TreeItem::new(TreeItemMode::Blob, Hash::default(), String::new());
    ///
    /// // Create a blob TreeItem with a custom Hash, and file name
    /// let file_item = TreeItem::new(TreeItemMode::Blob, Hash::new_from_str("1234567890abcdef1234567890abcdef12345678"), String::from("hello.txt"));
    ///
    /// // Create a tree TreeItem with a custom Hash, and directory name
    /// let dir_item = TreeItem::new(TreeItemMode::Tree, Hash::new_from_str("1234567890abcdef1234567890abcdef12345678"), String::from("data"));
    /// ```
    #[allow(unused)]
    pub fn new(mode: TreeItemMode, id: Hash, name: String) -> Self {
        TreeItem {
            mode,
            id,
            name,
        }
    }

    /// Create a new TreeItem from a byte vector, split into a mode, id and name, the TreeItem format is:
    ///
    /// ```bash
    /// <mode> <name>\0<binary object ID>
    /// ```
    ///
    /// # Example
    /// ```rust
    /// let bytes = Vec<u8>::new().to_bytes();
    //  let tree_item = TreeItem::new_from_bytes(bytes.as_slice()).unwrap();
    /// ```
    #[allow(unused)]
    pub fn new_from_bytes(bytes: &[u8]) -> Result<Self, GitError> {
        let mut parts = bytes.splitn(2, |b| *b == b' ');
        let mode = parts.next().unwrap();
        let rest = parts.next().unwrap();
        let mut parts = rest.splitn(2, |b| *b == b'\0');
        let name = parts.next().unwrap();
        let id = parts.next().unwrap();

        Ok(TreeItem {
            mode: TreeItemMode::tree_item_type_from(mode)?,
            id: Hash::from_bytes(id),
            name: String::from_utf8(name.to_vec())?,
        })
    }

    /// Convert a TreeItem to a byte vector
    /// ```rust
    /// let tree_item = super::TreeItem::new(
    ///     TreeItemMode::Blob,
    ///     Hash::new_from_str("8ab686eafeb1f44702738c8b0f24f2567c36da6d"),
    ///     "hello-world".to_string(),
    /// );
    ///
    /// let bytes = tree_item.to_bytes();
    /// ```
    #[allow(unused)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(self.mode.to_bytes());
        bytes.push(b' ');
        bytes.extend_from_slice(self.name.as_bytes());
        bytes.push(b'\0');
        bytes.extend_from_slice(&self.id.to_bytes());

        bytes
    }
}

/// A tree object is a Git object that represents a directory. It contains a list of entries, one
/// for each file or directory in the tree.
#[allow(unused)]
#[derive(PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Clone)]
pub struct Tree {
    pub meta: Meta,
    pub tree_items: Vec<TreeItem>,
}

impl Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Tree: {}", self.meta.id.to_string().blue())?;
        for item in &self.tree_items {
            write!(f, "{}", item)?;
        }

        Ok(())
    }
}

impl Tree {
    /// Create a new Tree with special hash id, **0000000000000000000000000000000000000000**
    #[allow(unused)]
    pub fn empty_tree_hash() -> Hash {
        Hash::default()
    }

    /// Generate a TreeItem from Tree object.
    /// This is used to generate a TreeItem for the parent directory of the current Tree object.
    #[allow(unused)]
    pub fn generate_self_2tree_item(&self, name: String) -> Result<TreeItem, GitError> {
        Ok(TreeItem::new(
            TreeItemMode::Tree,
            self.meta.id,
            name,
        ))
    }

    /// Create a new Tree from a Meta object
    #[allow(unused)]
    pub fn new_from_meta(meta: Meta) -> Result<Self, GitError> {
        let mut tree_items = Vec::new();

        let mut i = 0;
        while i < meta.size {
            let index = meta.data[i..].find_byte(0x00).unwrap();
            let next = i + index + 21;

            tree_items.push(TreeItem::new_from_bytes(
                &meta.data[i..next],
            )?);

            i = next
        }

        Ok(Tree {
            meta,
            tree_items,
        })
    }

    /// Crate a new Tree from a tree file
    #[allow(unused)]
    pub fn new_from_file(path: &str) -> Result<Self, GitError> {
        let meta = Meta::new_from_file(path)?;

        Tree::new_from_meta(meta)
    }

    /// Write to a file with path
    #[allow(unused)]
    pub fn write_2file(&self, path: &str) -> Result<String, GitError> {
        self.meta.loose_2file(path)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_tree_item_new() {
        use crate::git::hash::Hash;

        let tree_item = super::TreeItem::new(
            super::TreeItemMode::Blob,
            Hash::new_from_str("8ab686eafeb1f44702738c8b0f24f2567c36da6d"),
            "hello-world".to_string(),
        );

        assert_eq!(tree_item.mode, super::TreeItemMode::Blob);
        assert_eq!(tree_item.id.to_plain_str(), "8ab686eafeb1f44702738c8b0f24f2567c36da6d");
    }

    #[test]
    fn test_tree_item_to_bytes() {
        use crate::git::hash::Hash;

        let tree_item = super::TreeItem::new(
            super::TreeItemMode::Blob,
            Hash::new_from_str("8ab686eafeb1f44702738c8b0f24f2567c36da6d"),
            "hello-world".to_string(),
        );

        let bytes = tree_item.to_bytes();
        assert_eq!(bytes, vec![49, 48, 48, 54, 52, 52, 32, 104, 101, 108, 108, 111, 45, 119, 111, 114, 108, 100, 0, 138, 182, 134, 234, 254, 177, 244, 71, 2, 115, 140, 139, 15, 36, 242, 86, 124, 54, 218, 109]);
    }

    #[test]
    fn test_tree_item_from_bytes() {
        use crate::git::hash::Hash;

        let item = super::TreeItem::new(
            super::TreeItemMode::Blob,
            Hash::new_from_str("8ab686eafeb1f44702738c8b0f24f2567c36da6d"),
            "hello-world".to_string(),
        );

        let bytes = item.to_bytes();
        let tree_item = super::TreeItem::new_from_bytes(bytes.as_slice()).unwrap();

        assert_eq!(tree_item.mode, super::TreeItemMode::Blob);
        assert_eq!(tree_item.id.to_plain_str(), item.id.to_plain_str());
    }

    #[test]
    fn test_empty_tree_hash() {
        let hash = super::Tree::empty_tree_hash();
        assert_eq!(hash.to_plain_str(), "0000000000000000000000000000000000000000");
    }

    #[test]
    fn test_tree_new_from_file_with_one_item() {
        use std::env;
        use std::path::PathBuf;

        let mut source = PathBuf::from(env::current_dir().unwrap());
        source.push("tests/data/objects/f9/a1667a0dfce06819394c2aad557a04e9a13e56");

        let tree = super::Tree::new_from_file(source.to_str().unwrap()).unwrap();

        assert_eq!(tree.tree_items.len(), 1);
        assert_eq!(tree.tree_items[0].mode, super::TreeItemMode::Blob);
        assert_eq!(tree.tree_items[0].id.to_plain_str(), "8ab686eafeb1f44702738c8b0f24f2567c36da6d");
        assert_eq!(tree.tree_items[0].name, "hello-world");
        assert_eq!(tree.meta.id.to_plain_str(), "f9a1667a0dfce06819394c2aad557a04e9a13e56");
    }

    #[test]
    fn test_tree_new_from_file_with_two_items() {
        use std::env;
        use std::path::PathBuf;

        let mut source = PathBuf::from(env::current_dir().unwrap());
        source.push("tests/data/objects/e7/002dbbc79a209462247302c7757a31ab16df1e");

        let tree = super::Tree::new_from_file(source.to_str().unwrap()).unwrap();

        for item in tree.tree_items.iter() {
            if item.mode == super::TreeItemMode::Blob {
                assert_eq!(item.id.to_plain_str(), "8ab686eafeb1f44702738c8b0f24f2567c36da6d");
                assert_eq!(item.name, "hello-world");
            }

            if item.mode == super::TreeItemMode::Tree {
                assert_eq!(item.id.to_plain_str(), "c44c09a88097e5fb0c833d4178b2df78055ad2e9");
                assert_eq!(item.name, "rust");
            }
        }

        assert_eq!(tree.tree_items.len(), 2);
        assert_eq!(tree.meta.id.to_plain_str(), "e7002dbbc79a209462247302c7757a31ab16df1e");
    }

    #[test]
    fn test_tree_write_2file() {
        use std::env;
        use std::path::PathBuf;
        use std::fs::remove_file;

        use crate::git::internal::object::meta::Meta;
        use crate::git::internal::object::tree::Tree;

        let mut source = PathBuf::from(env::current_dir().unwrap());
        source.push("tests/data/objects/e7/002dbbc79a209462247302c7757a31ab16df1e");
        let meta = Meta::new_from_file(source.to_str().unwrap()).unwrap();
        let tree = Tree::new_from_meta(meta).unwrap();

        let mut dest_file = PathBuf::from(env::current_dir().unwrap());
        dest_file.push("tests/objects/e7/002dbbc79a209462247302c7757a31ab16df1e");
        if dest_file.exists() {
            remove_file(dest_file.as_path().to_str().unwrap()).unwrap();
        }

        let mut dest = PathBuf::from(env::current_dir().unwrap());
        dest.push("tests/objects");

        let path = tree.write_2file(dest.as_path().to_str().unwrap()).unwrap();

        assert_eq!(path, dest_file.as_path().to_str().unwrap());
    }
}