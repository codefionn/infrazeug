//! Ensure an S3 bucket exists (SigV4 against the OVH S3 endpoint).

use crate::client::OpenstackClientSource;
use async_trait::async_trait;
use infrazeug_ext_openstack::{bucket_exists, create_bucket};
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use infrazeug_secrets_s3::Credentials;
use serde::{Deserialize, Serialize};

pub const ENSURE_BUCKET: &str = "openstack.ensure_bucket";

pub type EnsureBucket = EnsureResource<BucketResource>;

pub fn ensure_bucket(source: OpenstackClientSource) -> EnsureBucket {
    EnsureResource::new(BucketResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureBucketInput {
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    /// Mutable vault file path (under `files/`, e.g. `mutable/cloud/backups.vault`).
    pub creds_file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureBucketOutput {
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
}

#[derive(Clone)]
pub struct BucketResource {
    #[allow(dead_code)]
    source: OpenstackClientSource,
}

impl BucketResource {
    pub fn new(source: OpenstackClientSource) -> Self {
        Self { source }
    }
}

async fn read_s3_creds(ctx: &ResourceCtx, creds_file: &str) -> ResourceResult<Credentials> {
    let access = ctx
        .read_secret_string(creds_file, "credentials.access_key")
        .await?;
    let secret = match ctx
        .read_secret_string(creds_file, "credentials.secret_key")
        .await
    {
        Ok(s) => s,
        Err(ResourceError::SecretsUnavailable) => return Err(ResourceError::SecretsUnavailable),
        Err(e) => return Err(e),
    };
    Ok(Credentials {
        access_key: access.trim().to_string(),
        secret_key: secret.trim().to_string(),
        session_token: None,
    })
}

#[async_trait]
impl Resource for BucketResource {
    type Spec = EnsureBucketInput;
    type State = EnsureBucketOutput;

    fn kind(&self) -> &'static str {
        ENSURE_BUCKET
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let creds = match read_s3_creds(ctx, &spec.creds_file).await {
            Ok(c) => c,
            Err(ResourceError::SecretsUnavailable) => {
                return Err(ResourceError::SecretsUnavailable)
            }
            Err(_) => return Ok(None),
        };
        let exists = bucket_exists(&spec.endpoint, &spec.region, &creds, &spec.bucket)
            .await
            .map_err(ResourceError::provider)?;
        if exists {
            Ok(Some(EnsureBucketOutput {
                bucket: spec.bucket.clone(),
                region: spec.region.clone(),
                endpoint: spec.endpoint.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let creds = read_s3_creds(ctx, &spec.creds_file).await?;
        create_bucket(&spec.endpoint, &spec.region, &creds, &spec.bucket)
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureBucketOutput {
            bucket: spec.bucket.clone(),
            region: spec.region.clone(),
            endpoint: spec.endpoint.clone(),
        })
    }

    fn diff(&self, _spec: &Self::Spec, _current: &Self::State) -> Drift {
        Drift::InSync
    }
}
