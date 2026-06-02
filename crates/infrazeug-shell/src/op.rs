use crate::source::{FileSource, PasswordHashSpec, RandomPasswordSpec};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

pub type Argv = Vec<String>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvVarSource {
    pub name: String,
    pub value: FileSource,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncDirOptions {
    /// Clear the destination directory before copying the source contents.
    pub delete: bool,
    /// Preserve hard links between files in the transferred tree.
    pub hard_links: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ShellOp {
    Run {
        argv: Argv,
        cwd: Option<PathBuf>,
        // NOTE: no `skip_serializing_if` here. Postcard is a non-self-describing
        // format: an omitted field has no marker on the wire, so the deserializer
        // reads it off the end of the buffer (DeserializeUnexpectedEnd). The field
        // must always be serialized — an empty Vec is just a single 0x00 length byte.
        #[serde(default)]
        env: Vec<EnvVarSource>,
    },
    Seq {
        steps: Vec<ShellOp>,
    },
    ReadFile {
        path: PathBuf,
    },
    WriteFile {
        path: PathBuf,
        content: FileSource,
        mode: u32,
    },
    /// Controller-side write of generated secret material into the infrazeug vault.
    VaultWrite {
        data_key_id: String,
        file: String,
        field: String,
        value: FileSource,
        /// Write-if-absent: an existing field is left untouched and the
        /// resolved value (e.g. a fresh random) is discarded. Set by the
        /// `*_random_*` constructors so generated secrets are stable across
        /// applies instead of rotating on every run.
        #[serde(default)]
        if_absent: bool,
    },
    /// Controller-side creation of a password and its PHC hash in the vault.
    /// Existing fields are preserved; missing fields are filled atomically.
    VaultEnsurePasswordHash {
        data_key_id: String,
        file: String,
        password_field: String,
        hash_field: String,
        password: RandomPasswordSpec,
        hash: PasswordHashSpec,
    },
    EnsureDir {
        path: PathBuf,
        mode: u32,
    },
    /// Sync a controller-local directory to the machine executing this op.
    SyncDir {
        src: PathBuf,
        dest: PathBuf,
        options: SyncDirOptions,
    },
    Poll {
        check_argv: Argv,
        every: Duration,
        timeout: Duration,
    },
}

impl ShellOp {
    pub fn run(argv: Argv) -> Self {
        Self::Run {
            argv,
            cwd: None,
            env: Vec::new(),
        }
    }

    pub fn run_with_env<I, N>(argv: Argv, env: I) -> Self
    where
        I: IntoIterator<Item = (N, FileSource)>,
        N: Into<String>,
    {
        Self::Run {
            argv,
            cwd: None,
            env: env
                .into_iter()
                .map(|(name, value)| EnvVarSource {
                    name: name.into(),
                    value,
                })
                .collect(),
        }
    }

    pub fn env(mut self, name: impl Into<String>, value: FileSource) -> Self {
        if let Self::Run { env, .. } = &mut self {
            env.push(EnvVarSource {
                name: name.into(),
                value,
            });
        }
        self
    }

    pub fn read_file(path: impl Into<PathBuf>) -> Self {
        Self::ReadFile { path: path.into() }
    }

    pub fn write_file(path: impl Into<PathBuf>, content: FileSource, mode: u32) -> Self {
        Self::WriteFile {
            path: path.into(),
            content,
            mode,
        }
    }

    pub fn write_file_bytes(
        path: impl Into<PathBuf>,
        content: impl Into<Vec<u8>>,
        mode: u32,
    ) -> Self {
        Self::WriteFile {
            path: path.into(),
            content: FileSource::bytes(content),
            mode,
        }
    }

    pub fn write_random_bytes(path: impl Into<PathBuf>, len: usize, mode: u32) -> Self {
        Self::write_file(path, FileSource::random_bytes(len), mode)
    }

    pub fn write_random_base64(path: impl Into<PathBuf>, byte_len: usize, mode: u32) -> Self {
        Self::write_file(path, FileSource::random_base64(byte_len), mode)
    }

    pub fn write_random_password(
        path: impl Into<PathBuf>,
        spec: RandomPasswordSpec,
        mode: u32,
    ) -> Self {
        Self::write_file(path, FileSource::random_password(spec), mode)
    }

    pub fn sync_dir(src: impl Into<PathBuf>, dest: impl Into<PathBuf>) -> Self {
        Self::SyncDir {
            src: src.into(),
            dest: dest.into(),
            options: SyncDirOptions::default(),
        }
    }

    pub fn sync_dir_with_options(
        src: impl Into<PathBuf>,
        dest: impl Into<PathBuf>,
        options: SyncDirOptions,
    ) -> Self {
        Self::SyncDir {
            src: src.into(),
            dest: dest.into(),
            options,
        }
    }

    pub fn vault_write(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        value: FileSource,
    ) -> Self {
        Self::VaultWrite {
            data_key_id: data_key_id.into(),
            file: file.into(),
            field: field.into(),
            value,
            if_absent: false,
        }
    }

    pub fn mutable_vault_write(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        value: FileSource,
    ) -> Self {
        Self::VaultWrite {
            data_key_id: data_key_id.into(),
            file: infrazeug_secrets::mutable_vault_path(file),
            field: field.into(),
            value,
            if_absent: false,
        }
    }

    fn vault_write_if_absent(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        value: FileSource,
    ) -> Self {
        Self::VaultWrite {
            data_key_id: data_key_id.into(),
            file: file.into(),
            field: field.into(),
            value,
            if_absent: true,
        }
    }

    /// Write-if-absent: a fresh random is generated each apply but an existing
    /// field is never overwritten, so the stored secret is stable.
    pub fn vault_write_random_bytes(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        len: usize,
    ) -> Self {
        Self::vault_write_if_absent(data_key_id, file, field, FileSource::random_bytes(len))
    }

    /// Write-if-absent: a fresh random is generated each apply but an existing
    /// field is never overwritten, so the stored secret is stable.
    pub fn vault_write_random_base64(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        byte_len: usize,
    ) -> Self {
        Self::vault_write_if_absent(
            data_key_id,
            file,
            field,
            FileSource::random_base64(byte_len),
        )
    }

    /// Write-if-absent: a fresh random is generated each apply but an existing
    /// field is never overwritten, so the stored secret is stable.
    pub fn vault_write_random_password(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        spec: RandomPasswordSpec,
    ) -> Self {
        Self::vault_write_if_absent(data_key_id, file, field, FileSource::random_password(spec))
    }

    /// Write-if-absent: a fresh random is generated each apply but an existing
    /// field is never overwritten, so the stored secret is stable.
    pub fn mutable_vault_write_random_bytes(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        len: usize,
    ) -> Self {
        Self::vault_write_if_absent(
            data_key_id,
            infrazeug_secrets::mutable_vault_path(file),
            field,
            FileSource::random_bytes(len),
        )
    }

    /// Write-if-absent: a fresh random is generated each apply but an existing
    /// field is never overwritten, so the stored secret is stable.
    pub fn mutable_vault_write_random_base64(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        byte_len: usize,
    ) -> Self {
        Self::vault_write_if_absent(
            data_key_id,
            infrazeug_secrets::mutable_vault_path(file),
            field,
            FileSource::random_base64(byte_len),
        )
    }

    /// Write-if-absent: a fresh random is generated each apply but an existing
    /// field is never overwritten, so the stored secret is stable.
    pub fn mutable_vault_write_random_password(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        field: impl Into<String>,
        spec: RandomPasswordSpec,
    ) -> Self {
        Self::vault_write_if_absent(
            data_key_id,
            infrazeug_secrets::mutable_vault_path(file),
            field,
            FileSource::random_password(spec),
        )
    }

    pub fn vault_ensure_random_password_hash(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        password_field: impl Into<String>,
        hash_field: impl Into<String>,
        password: RandomPasswordSpec,
        hash: PasswordHashSpec,
    ) -> Self {
        Self::VaultEnsurePasswordHash {
            data_key_id: data_key_id.into(),
            file: file.into(),
            password_field: password_field.into(),
            hash_field: hash_field.into(),
            password,
            hash,
        }
    }

    pub fn mutable_vault_ensure_random_password_hash(
        data_key_id: impl Into<String>,
        file: impl Into<String>,
        password_field: impl Into<String>,
        hash_field: impl Into<String>,
        password: RandomPasswordSpec,
        hash: PasswordHashSpec,
    ) -> Self {
        Self::vault_ensure_random_password_hash(
            data_key_id,
            infrazeug_secrets::mutable_vault_path(file),
            password_field,
            hash_field,
            password,
            hash,
        )
    }

    pub fn poll(check_argv: Argv, every: Duration, timeout: Duration) -> Self {
        Self::Poll {
            check_argv,
            every,
            timeout,
        }
    }
}
