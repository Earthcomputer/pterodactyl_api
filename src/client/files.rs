//! API for endpoints under `api/client/servers/{server}/files`

use crate::client::Server;
use crate::http::{EmptyBody, NullErrorHandler, RawBody};
use crate::structs::{PteroList, PteroObject};
use bytes::Bytes;
use reqwest::{Body, Method};
use serde::{Deserialize, Deserializer, Serialize};
use time::OffsetDateTime;

fn split_dir_filename(file: &str) -> (&str, &str) {
    file.split_at(file.rfind('/').map_or(0, |i| i + 1))
}

fn split_filename_extension(file: &str) -> (&str, &str) {
    file.split_at(file.rfind('.').unwrap_or(file.len()))
}

fn file_parts(mut file: &str) -> impl Iterator<Item = &str> {
    if file.starts_with('/') {
        file = &file[1..];
    }
    if file.ends_with('/') {
        file = &file[..file.len() - 1];
    }
    file.split('/').skip(if file.is_empty() { 1 } else { 0 })
}

fn relativize(file: &str, root: &str) -> String {
    let (dir, filename) = split_dir_filename(file);

    let mut root_parts = file_parts(root);
    let mut file_parts = file_parts(dir);
    let mut root_part = root_parts.next();
    let mut file_part = file_parts.next();

    // skip over common prefix
    while root_part.is_some() && root_part == file_part {
        root_part = root_parts.next();
        file_part = file_parts.next();
    }

    // go up the parent directories out of root
    let mut result = String::new();
    while root_part.is_some() {
        result.push_str("../");
        root_part = root_parts.next();
    }

    // go back down some directories towards file
    while let Some(part) = file_part {
        result.push_str(part);
        result.push('/');
        file_part = file_parts.next();
    }

    // finally, push the filename
    result.push_str(filename);

    result
}

/// Represents a file on the file system of a Pterodactyl server
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct PteroFile {
    /// The file name
    pub name: String,

    /// The Unix permissions of the file
    #[serde(rename = "mode")]
    pub permissions: PteroFilePermissions,

    /// The size of the file in bytes
    pub size: u64,

    /// Whether this is a normal file (as opposed to a directory or symlink)
    pub is_file: bool,

    /// Whether this is a symlink
    #[serde(default)]
    pub is_symlink: bool,

    /// Whether this file is editable
    #[serde(default = "crate::structs::bool_true")]
    pub is_editable: bool,

    /// The mimetype of the file
    pub mimetype: String,

    /// When the file was created
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub created_at: OffsetDateTime,

    /// When the file was last modified
    #[serde(deserialize_with = "crate::structs::iso_time")]
    pub modified_at: OffsetDateTime,
}

/// The file type of a Pterodactyl file
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum PteroFileType {
    /// A normal file
    Normal,
    /// A directory
    Directory,
    /// A symlink
    Symlink,
}

/// The Unix permissions of a Pterodactyl file
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct PteroFilePermissions {
    /// The type of the file
    pub file_type: PteroFileType,
    /// The permissions for the owner of the file
    pub owner: PteroUserFilePermissions,
    /// The permissions for the group owner of the file
    pub group_owner: PteroUserFilePermissions,
    /// The permissions for other users
    pub other_users: PteroUserFilePermissions,
}

/// The Unix permissions of a Pterodactyl file for a specific user
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct PteroUserFilePermissions {
    /// Whether the user has read access to this file
    pub read: bool,
    /// Whether the user has write access to this file
    pub write: bool,
    /// Whether this file is executable for the user
    pub executable: bool,
    /// Whether this file has the setuid/setgid flag set
    pub setuid: bool,
    /// Whether this file has the sticky flag set
    pub sticky: bool,
}

impl<'de> Deserialize<'de> for PteroFilePermissions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string: String = Deserialize::deserialize(deserializer)?;
        let mut chars = string.chars();
        macro_rules! next_char {
            ($chars:expr, $($lit:literal => $value:expr),*) => {{
                match $chars.next() {
                    $(
                    Some($lit) => $value,
                    )*
                    Some(c) => {
                        return Err(<D::Error as serde::de::Error>::custom(format!(
                            concat!("Expected {}, found {}"),
                            [$($lit),*].map(|c| format!("'{c}'")).into_iter().collect::<Vec<_>>().join("/"), c
                        )))
                    }
                    None => {
                        return Err(<D::Error as serde::de::Error>::custom(
                            "File permissions must be of length 10",
                        ))
                    }
                }
            }};
        }

        fn read_user<'de, D: Deserializer<'de>>(
            chars: &mut impl Iterator<Item = char>,
        ) -> Result<PteroUserFilePermissions, D::Error> {
            let read = next_char!(chars, 'r' => true, '-' => false);
            let write = next_char!(chars, 'w' => true, '-' => false);
            let (executable, setuid, sticky) = next_char!(chars,
                'x' => (true, false, false),
                's' => (true, true, false),
                't' => (true, false, true),
                'S' => (false, true, false),
                'T' => (false, false, true),
                '-' => (false, false, false)
            );
            Ok(PteroUserFilePermissions {
                read,
                write,
                executable,
                setuid,
                sticky,
            })
        }

        let result = PteroFilePermissions {
            file_type: next_char!(chars,
                'd' => PteroFileType::Directory,
                'l' => PteroFileType::Symlink,
                '-' => PteroFileType::Normal
            ),
            owner: read_user::<D>(&mut chars)?,
            group_owner: read_user::<D>(&mut chars)?,
            other_users: read_user::<D>(&mut chars)?,
        };
        if chars.next().is_some() {
            return Err(<D::Error as serde::de::Error>::custom(
                "File permissions must be of length 10",
            ));
        }
        Ok(result)
    }
}

impl Server<'_> {
    /// Lists the files in a directory on the server
    pub async fn list_files(&self, directory: impl AsRef<str>) -> crate::Result<Vec<PteroFile>> {
        self.client
            .request::<PteroList<PteroFile>>(
                Method::GET,
                &format!(
                    "servers/{}/files/list?directory={}",
                    self.id,
                    urlencoding::encode(directory.as_ref())
                ),
            )
            .await
            .map(|files| files.data)
    }

    /// Gets the file contents of a file on the server, as a [`Bytes`]
    pub async fn file_contents(&self, file: impl AsRef<str>) -> crate::Result<Bytes> {
        Ok(self
            .client
            .get_response::<_, NullErrorHandler>(
                Method::GET,
                &format!(
                    "servers/{}/files/contents?file={}",
                    self.id,
                    urlencoding::encode(file.as_ref())
                ),
                EmptyBody,
            )
            .await?
            .bytes()
            .await?)
    }

    /// Gets the file contents of a UTF8-encoded file on the server, as a [`String`]
    pub async fn file_contents_text(&self, file: impl AsRef<str>) -> crate::Result<String> {
        Ok(self
            .client
            .get_response::<_, NullErrorHandler>(
                Method::GET,
                &format!(
                    "servers/{}/files/contents?file={}",
                    self.id,
                    urlencoding::encode(file.as_ref())
                ),
                EmptyBody,
            )
            .await?
            .text()
            .await?)
    }

    /// Streams the file contents of a  file on the server, as an async stream
    #[cfg(feature = "stream")]
    pub async fn file_contents_stream(
        &self,
        file: impl AsRef<str>,
    ) -> crate::Result<impl futures_core::Stream<Item = reqwest::Result<Bytes>>> {
        Ok(self
            .client
            .get_response::<_, NullErrorHandler>(
                Method::GET,
                &format!(
                    "servers/{}/files/contents?file={}",
                    self.id,
                    urlencoding::encode(file.as_ref())
                ),
                EmptyBody,
            )
            .await?
            .bytes_stream())
    }

    /// Gets a one-time download URL for a file on the server
    pub async fn get_file_download_url(&self, file: impl AsRef<str>) -> crate::Result<String> {
        #[derive(Deserialize)]
        struct Url {
            url: String,
        }
        self.client
            .request::<PteroObject<Url>>(
                Method::GET,
                &format!(
                    "servers/{}/files/download?file={}",
                    self.id,
                    urlencoding::encode(file.as_ref())
                ),
            )
            .await
            .map(|url| url.attributes.url)
    }

    /// Renames or moves a file on the server
    pub async fn rename_file(
        &self,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> crate::Result<()> {
        self.rename_files(vec![(from.into(), to.into())]).await
    }

    /// Bulk renames or moves files on this server. Takes a set of pairs, the first element is the
    /// file to rename from, and the second element is what to rename into.
    pub async fn rename_files(&self, files: Vec<(String, String)>) -> crate::Result<()> {
        #[derive(Serialize)]
        struct FileRename {
            from: String,
            to: String,
        }
        #[derive(Serialize)]
        struct RenameFilesBody {
            root: &'static str,
            files: Vec<FileRename>,
        }
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::PUT,
                &format!("servers/{}/files/rename", self.id),
                &RenameFilesBody {
                    root: "/",
                    files: files
                        .into_iter()
                        .map(|(from, to)| FileRename { from, to })
                        .collect(),
                },
            )
            .await?;
        Ok(())
    }

    /// Creates a copy of a file (or directory) on this server.
    ///
    /// If the original file has the format `[directory/]filename[.extension]`, then the copy will
    /// have the format `[directory/]filename copy[ number][.extension]`, where `number` is first
    /// absent, and then incremented starting from 1 until the file doesn't exist yet.
    ///
    /// To be able to choose the name of the copy, use [`Server::copy_file`] instead.
    pub async fn create_file_copy(&self, file: impl Into<String>) -> crate::Result<()> {
        #[derive(Serialize)]
        struct CreateCopyBody {
            location: String,
        }
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::POST,
                &format!("servers/{}/files/copy", self.id),
                &CreateCopyBody {
                    location: file.into(),
                },
            )
            .await?;
        Ok(())
    }

    /// Copies a file (or directory) on this server to the given destination location.
    pub async fn copy_file(
        &self,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> crate::Result<()> {
        let from = from.into();

        let (dir, filename) = split_dir_filename(&from);
        let files = self.list_files(dir).await?;
        let (filename, extension) = split_filename_extension(filename);

        let mut copy_name = format!("{}{} copy{}", dir, filename, extension);
        let mut i = 0usize;
        while files.iter().any(|file| file.name == copy_name) {
            i += 1;
            copy_name = format!("{}{} copy {}{}", dir, filename, i, extension);
        }
        let copy_name = format!("{}{}", dir, copy_name);

        self.create_file_copy(from).await?;
        self.rename_file(copy_name, to).await?;

        Ok(())
    }

    /// Overwrites the given file on this server with the given data
    pub async fn write_file(
        &self,
        file: impl AsRef<str>,
        data: impl Into<Body>,
    ) -> crate::Result<()> {
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::POST,
                &format!(
                    "servers/{}/files/write?file={}",
                    self.id,
                    urlencoding::encode(file.as_ref())
                ),
                RawBody(data),
            )
            .await?;
        Ok(())
    }

    /// Compresses a file (or directory) on this server into a tarball (`.tar.gz`)
    pub async fn compress_file(&self, file: impl Into<String>) -> crate::Result<PteroFile> {
        let file = file.into();
        let (dir, filename) = split_dir_filename(&file);
        self.compress_files(dir, vec![filename.to_owned()]).await
    }

    /// Compresses a set of files (or directories) on this server into a tarball (`.tar.gz`)
    pub async fn compress_files(
        &self,
        root: impl Into<String>,
        files: Vec<String>,
    ) -> crate::Result<PteroFile> {
        #[derive(Serialize)]
        struct CompressFilesBody {
            root: String,
            files: Vec<String>,
        }
        self.client
            .request_with_body::<PteroObject<PteroFile>, _>(
                Method::POST,
                &format!("servers/{}/files/compress", self.id),
                &CompressFilesBody {
                    root: root.into(),
                    files,
                },
            )
            .await
            .map(|file| file.attributes)
    }

    /// Decompresses a tarball (`.tar.gz`) into the specified destination directory on this server
    pub async fn decompress_file(
        &self,
        file: impl Into<String>,
        dest: impl Into<String>,
    ) -> crate::Result<()> {
        #[derive(Serialize)]
        struct DecompressBody {
            root: String,
            file: String,
        }
        let dest = dest.into();
        let file = relativize(&file.into(), &dest);
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::POST,
                &format!("servers/{}/files/decompress", self.id),
                &DecompressBody { root: dest, file },
            )
            .await?;
        Ok(())
    }

    /// Deletes the given file or directory on this server
    pub async fn delete_file(&self, file: impl Into<String>) -> crate::Result<()> {
        self.delete_files(vec![file.into()]).await
    }

    /// Bulk deletes the given files or directories on this server
    pub async fn delete_files(&self, files: Vec<String>) -> crate::Result<()> {
        #[derive(Serialize)]
        struct DeleteFilesBody {
            root: &'static str,
            files: Vec<String>,
        }
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::POST,
                &format!("servers/{}/files/delete", self.id),
                &DeleteFilesBody { root: "/", files },
            )
            .await?;
        Ok(())
    }

    /// Creates a folder on this server
    pub async fn create_folder(&self, folder: impl Into<String>) -> crate::Result<()> {
        #[derive(Serialize)]
        struct CreateFolderBody {
            root: String,
            name: String,
        }
        let folder = folder.into();
        let mut folder_ref = &folder[..];
        if folder_ref.ends_with('/') {
            folder_ref = &folder_ref[..folder_ref.len() - 1];
        }
        let (dir, folder_name) = split_dir_filename(folder_ref);
        self.client
            .request_with_body::<EmptyBody, _>(
                Method::POST,
                &format!("servers/{}/files/create-folder", self.id),
                &CreateFolderBody {
                    root: dir.to_owned(),
                    name: folder_name.to_owned(),
                },
            )
            .await?;
        Ok(())
    }

    /// Gets a temporary upload URL to upload files
    pub async fn get_files_upload_url(&self) -> crate::Result<String> {
        #[derive(Deserialize)]
        struct Url {
            url: String,
        }
        self.client
            .request::<PteroObject<Url>>(Method::GET, &format!("servers/{}/files/upload", self.id,))
            .await
            .map(|url| url.attributes.url)
    }
}
