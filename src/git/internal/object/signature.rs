//! In a Git commit, the author signature contains the name, email address, timestamp, and timezone
//! of the person who authored the commit. This information is stored in a specific format, which
//! consists of the following fields:
//!
//! - Name: The name of the author, encoded as a UTF-8 string.
//! - Email: The email address of the author, encoded as a UTF-8 string.
//! - Timestamp: The timestamp of when the commit was authored, encoded as a decimal number of seconds
//! since the Unix epoch (January 1, 1970, 00:00:00 UTC).
//! - Timezone: The timezone offset of the author's local time from Coordinated Universal Time (UTC),
//! encoded as a string in the format "+HHMM" or "-HHMM".
//!
use std::fmt::Display;

use bstr::ByteSlice;

use crate::git::errors::GitError;

/// In addition to the author signature, Git also includes a "committer" signature, which indicates
/// who committed the changes to the repository. The committer signature is similar in structure to
/// the author signature, but includes the name, email address, and timestamp of the committer instead.
/// This can be useful in situations where multiple people are working on a project and changes are
/// being reviewed and merged by someone other than the original author.
///
/// In the following example, it's has the only one who authored and committed.
/// ```bash
/// author Eli Ma <eli@patch.sh> 1678102132 +0800
/// committer Quanyi Ma <eli@patch.sh> 1678102132 +0800
/// ```
///
/// So, we design a `SignatureType` enum to indicate the signature type.
#[derive(PartialEq, Eq, Debug, Hash, Ord, PartialOrd, Clone)]
pub enum SignatureType {
    Author,
    Committer,
}

impl Display for SignatureType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SignatureType::Author => write!(f, "Author"),
            SignatureType::Committer => write!(f, "Committer"),
        }
    }
}

impl SignatureType {
    /// The `from_str` method is used to convert a string to a `SignatureType` enum.
    #[allow(unused)]
    pub fn from_str(s: &str) -> Result<Self, GitError> {
        match s {
            "author" => Ok(SignatureType::Author),
            "committer" => Ok(SignatureType::Committer),
            _ => Err(GitError::InvalidSignatureType(s.to_string())),
        }
    }

    /// The `from_data` method is used to convert a `Vec<u8>` to a `SignatureType` enum.
    #[allow(unused)]
    pub fn from_data(data: Vec<u8>) -> Result<Self, GitError> {
        let s = String::from_utf8(data.to_vec())?;
        SignatureType::from_str(s.as_str())
    }

    /// The `to_bytes` method is used to convert a `SignatureType` enum to a `Vec<u8>`.
    #[allow(unused)]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SignatureType::Author => "author".to_string().into_bytes(),
            SignatureType::Committer => "committer".to_string().into_bytes(),
        }
    }
}

#[allow(unused)]
pub struct Signature {
    pub signature_type: SignatureType,
    pub name: String,
    pub email: String,
    pub timestamp: usize,
    pub timezone: String,
}

impl Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "{} <{}>", self.name, self.email).unwrap();
        writeln!(f, "Date: {}", self.timestamp).unwrap();
        writeln!(f, "{}", self.signature_type)
    }
}

impl Signature {
    #[allow(unused)]
    pub fn new_from_data(&mut self, data: Vec<u8>) -> Result<(), GitError> {
        // Make a mutable copy of the input data vector.
        let mut sign = data;

        // Find the index of the first space byte in the data vector.
        let name_start = sign.find_byte(0x20).unwrap();

        // Parse the author name from the bytes up to the first space byte.
        // If the parsing fails, unwrap will panic.
        self.signature_type = SignatureType::from_data(sign[..name_start].to_vec()).unwrap();

        // Find the indices of the email address bytes within the data vector.
        let email_start = sign.find_byte(0x3C).unwrap();
        let email_end = sign.find_byte(0x3E).unwrap();

        // Parse the name and email address from the data vector using slicing and string conversion.
        // If the parsing fails, unwrap will panic.
        self.name = sign[name_start + 1..email_start - 1]
            .to_str()
            .unwrap()
            .to_string();

        self.email = sign[email_start + 1..email_end]
            .to_str()
            .unwrap()
            .to_string();

        // Update the data vector to remove the author and email bytes.
        sign = sign[email_end + 2..].to_vec();

        // Find the index of the second space byte in the updated data vector.
        let timestamp_split = sign.find_byte(0x20).unwrap();

        // Parse the timestamp integer from the bytes up to the second space byte.
        // If the parsing fails, unwrap will panic.
        self.timestamp = sign[0..timestamp_split]
            .to_str()
            .unwrap()
            .parse::<usize>()
            .unwrap();

        // Parse the timezone string from the bytes after the second space byte.
        // If the parsing fails, unwrap will panic.
        self.timezone = sign[timestamp_split + 1..].to_str().unwrap().to_string();

        // Return a Result object indicating success.
        Ok(())
    }

    ///
    #[allow(unused)]
    pub fn to_data(&self) -> Result<Vec<u8>, GitError> {
        // Create a new empty vector to store the encoded data.
        let mut sign = Vec::new();

        // Append the author name bytes to the data vector, followed by a space byte.
        sign.extend_from_slice(&self.signature_type.to_bytes());
        sign.extend_from_slice(0x20u8.to_be_bytes().as_ref());

        // Append the name bytes to the data vector, followed by a space byte.
        sign.extend_from_slice(self.name.as_bytes());
        sign.extend_from_slice(0x20u8.to_be_bytes().as_ref());

        // Append the email address bytes to the data vector, enclosed in angle brackets.
        sign.extend_from_slice(0x3Cu8.to_be_bytes().as_ref());
        sign.extend_from_slice(self.email.as_bytes());
        sign.extend_from_slice(0x3Eu8.to_be_bytes().as_ref());

        // Append the timestamp integer bytes to the data vector, followed by a space byte.
        sign.extend_from_slice(0x20u8.to_be_bytes().as_ref());
        sign.extend_from_slice(self.timestamp.to_string().as_bytes());
        sign.extend_from_slice(0x20u8.to_be_bytes().as_ref());

        // Append the timezone string bytes to the data vector.
        sign.extend_from_slice(self.timezone.as_bytes());

        // Return the data vector as a Result object indicating success.
        Ok(sign)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_signature_type_from_str() {
        assert_eq!(
            super::SignatureType::from_str("author").unwrap(),
            super::SignatureType::Author
        );

        assert_eq!(
            super::SignatureType::from_str("committer").unwrap(),
            super::SignatureType::Committer
        );
    }

    #[test]
    fn test_signature_type_from_data() {
        assert_eq!(
            super::SignatureType::from_data("author".to_string().into_bytes()).unwrap(),
            super::SignatureType::Author
        );

        assert_eq!(
            super::SignatureType::from_data("committer".to_string().into_bytes()).unwrap(),
            super::SignatureType::Committer
        );
    }

    #[test]
    fn test_signature_type_to_bytes() {
        assert_eq!(
            super::SignatureType::Author.to_bytes(),
            "author".to_string().into_bytes()
        );

        assert_eq!(
            super::SignatureType::Committer.to_bytes(),
            "committer".to_string().into_bytes()
        );
    }

    #[test]
    fn test_signature_new_from_data() {
        let mut sign = super::Signature {
            signature_type: super::SignatureType::Author,
            name: String::new(),
            email: String::new(),
            timestamp: 0,
            timezone: String::new(),
        };

        sign.new_from_data(
            "author Quanyi Ma <eli@patch.sh> 1678101573 +0800".to_string().into_bytes()).unwrap();

        assert_eq!(sign.signature_type, super::SignatureType::Author);
        assert_eq!(sign.name, "Quanyi Ma");
        assert_eq!(sign.email, "eli@patch.sh");
        assert_eq!(sign.timestamp, 1678101573);
        assert_eq!(sign.timezone, "+0800");
    }

    #[test]
    fn test_signature_to_data() {
        let mut sign = super::Signature {
            signature_type: super::SignatureType::Author,
            name: String::new(),
            email: String::new(),
            timestamp: 0,
            timezone: String::new(),
        };

        sign.new_from_data(
            "committer Quanyi Ma <eli@patch.sh> 1678101573 +0800".to_string().into_bytes()).unwrap();

        let dest = sign.to_data().unwrap();

        assert_eq!(dest, "committer Quanyi Ma <eli@patch.sh> 1678101573 +0800".to_string().into_bytes());
    }

}