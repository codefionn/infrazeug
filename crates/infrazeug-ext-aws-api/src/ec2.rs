//! EC2 compute and EBS volume operations (Query API).

use crate::client::{ensure_success, AwsClient};
use crate::error::{AwsError, Result};
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};

const EC2_VERSION: &str = "2016-11-15";

/// EC2 instance summary.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Instance {
    pub instance_id: String,
    pub name: String,
    pub state: Option<String>,
    #[serde(default)]
    pub private_ip: Option<String>,
    #[serde(default)]
    pub public_ip: Option<String>,
}

/// EBS volume summary.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Volume {
    pub volume_id: String,
    pub name: String,
    pub size: u32,
    pub volume_type: Option<String>,
}

/// Body for launching an EC2 instance.
#[derive(Debug, Clone, Default, Serialize)]
pub struct InstanceCreate {
    pub image_id: String,
    pub instance_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_group_ids: Option<Vec<String>>,
}

/// Body for creating an EBS volume.
#[derive(Debug, Clone, Default, Serialize)]
pub struct VolumeCreate {
    pub name: String,
    pub availability_zone: String,
    pub size: u32,
    pub volume_type: String,
}

impl AwsClient {
    /// `DescribeInstances` for a single instance ID.
    pub async fn ec2_instance(&self, instance_id: &str) -> Result<Option<Instance>> {
        let params = vec![
            ("Action".into(), "DescribeInstances".into()),
            ("Version".into(), EC2_VERSION.into()),
            ("InstanceId.1".into(), instance_id.into()),
        ];
        let (status, body) = self.ec2_query(&params).await?;
        ensure_success(status, &body)?;
        Ok(parse_instances(&body)?.into_iter().next())
    }

    /// `DescribeInstances` filtered by `tag:Name`.
    pub async fn ec2_instances(&self, name: &str) -> Result<Vec<Instance>> {
        let params = vec![
            ("Action".into(), "DescribeInstances".into()),
            ("Version".into(), EC2_VERSION.into()),
            ("Filter.1.Name".into(), "tag:Name".into()),
            ("Filter.1.Value.1".into(), name.into()),
        ];
        let (status, body) = self.ec2_query(&params).await?;
        ensure_success(status, &body)?;
        parse_instances(&body)
    }

    /// `RunInstances` with a `Name` tag.
    pub async fn ec2_instance_create(&self, create: &InstanceCreate) -> Result<Instance> {
        let mut params = vec![
            ("Action".into(), "RunInstances".into()),
            ("Version".into(), EC2_VERSION.into()),
            ("ImageId".into(), create.image_id.clone()),
            ("InstanceType".into(), create.instance_type.clone()),
            ("MinCount".into(), "1".into()),
            ("MaxCount".into(), "1".into()),
            ("TagSpecification.1.ResourceType".into(), "instance".into()),
            ("TagSpecification.1.Tag.1.Key".into(), "Name".into()),
            ("TagSpecification.1.Tag.1.Value".into(), create.name.clone()),
        ];
        if let Some(key) = &create.key_name {
            params.push(("KeyName".into(), key.clone()));
        }
        if let Some(subnet) = &create.subnet_id {
            params.push(("SubnetId".into(), subnet.clone()));
        }
        if let Some(groups) = &create.security_group_ids {
            for (i, sg) in groups.iter().enumerate() {
                params.push((format!("SecurityGroupId.{}", i + 1), sg.clone()));
            }
        }
        let (status, body) = self.ec2_query(&params).await?;
        ensure_success(status, &body)?;
        parse_instances(&body)?
            .into_iter()
            .next()
            .ok_or_else(|| AwsError::Api {
                status: status.as_u16(),
                message: "RunInstances returned no instance".into(),
            })
    }

    /// `DescribeVolumes` for a single volume ID.
    pub async fn ec2_volume(&self, volume_id: &str) -> Result<Option<Volume>> {
        let params = vec![
            ("Action".into(), "DescribeVolumes".into()),
            ("Version".into(), EC2_VERSION.into()),
            ("VolumeId.1".into(), volume_id.into()),
        ];
        let (status, body) = self.ec2_query(&params).await?;
        ensure_success(status, &body)?;
        Ok(parse_volumes(&body)?.into_iter().next())
    }

    /// `DescribeVolumes` filtered by `tag:Name`.
    pub async fn ec2_volumes(&self, name: &str) -> Result<Vec<Volume>> {
        let params = vec![
            ("Action".into(), "DescribeVolumes".into()),
            ("Version".into(), EC2_VERSION.into()),
            ("Filter.1.Name".into(), "tag:Name".into()),
            ("Filter.1.Value.1".into(), name.into()),
        ];
        let (status, body) = self.ec2_query(&params).await?;
        ensure_success(status, &body)?;
        parse_volumes(&body)
    }

    /// `CreateVolume` with a `Name` tag.
    pub async fn ec2_volume_create(&self, create: &VolumeCreate) -> Result<Volume> {
        let params = vec![
            ("Action".into(), "CreateVolume".into()),
            ("Version".into(), EC2_VERSION.into()),
            ("AvailabilityZone".into(), create.availability_zone.clone()),
            ("Size".into(), create.size.to_string()),
            ("VolumeType".into(), create.volume_type.clone()),
            ("TagSpecification.1.ResourceType".into(), "volume".into()),
            ("TagSpecification.1.Tag.1.Key".into(), "Name".into()),
            ("TagSpecification.1.Tag.1.Value".into(), create.name.clone()),
        ];
        let (status, body) = self.ec2_query(&params).await?;
        ensure_success(status, &body)?;
        parse_volumes(&body)?
            .into_iter()
            .next()
            .ok_or_else(|| AwsError::Api {
                status: status.as_u16(),
                message: "CreateVolume returned no volume".into(),
            })
    }
}

#[derive(Debug, Deserialize)]
struct TagItem {
    #[serde(rename = "key")]
    key: Option<String>,
    #[serde(rename = "value")]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TagSet {
    #[serde(default, rename = "item")]
    items: Vec<TagItem>,
}

#[derive(Debug, Deserialize)]
struct InstanceItem {
    #[serde(rename = "instanceId")]
    instance_id: Option<String>,
    #[serde(rename = "instanceState")]
    instance_state: Option<StateItem>,
    #[serde(rename = "privateIpAddress")]
    private_ip: Option<String>,
    #[serde(rename = "ipAddress")]
    public_ip: Option<String>,
    #[serde(rename = "tagSet")]
    tag_set: Option<TagSet>,
}

#[derive(Debug, Deserialize)]
struct StateItem {
    #[serde(rename = "name")]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InstancesSet {
    #[serde(default, rename = "item")]
    items: Vec<InstanceItem>,
}

#[derive(Debug, Deserialize)]
struct ReservationItem {
    #[serde(rename = "instancesSet")]
    instances_set: Option<InstancesSet>,
}

#[derive(Debug, Deserialize)]
struct ReservationSet {
    #[serde(default, rename = "item")]
    items: Vec<ReservationItem>,
}

#[derive(Debug, Deserialize)]
struct DescribeInstancesResponse {
    #[serde(rename = "reservationSet")]
    reservation_set: Option<ReservationSet>,
}

#[derive(Debug, Deserialize)]
struct VolumeItem {
    #[serde(rename = "volumeId")]
    volume_id: Option<String>,
    #[serde(rename = "size")]
    size: Option<String>,
    #[serde(rename = "volumeType")]
    volume_type: Option<String>,
    #[serde(rename = "tagSet")]
    tag_set: Option<TagSet>,
}

#[derive(Debug, Deserialize)]
struct VolumeSet {
    #[serde(default, rename = "item")]
    items: Vec<VolumeItem>,
}

#[derive(Debug, Deserialize)]
struct DescribeVolumesResponse {
    #[serde(rename = "volumeSet")]
    volume_set: Option<VolumeSet>,
}

#[derive(Debug, Deserialize)]
struct CreateVolumeResponse {
    #[serde(rename = "volumeId")]
    volume_id: Option<String>,
    #[serde(rename = "size")]
    size: Option<String>,
    #[serde(rename = "volumeType")]
    volume_type: Option<String>,
    #[serde(rename = "tagSet")]
    tag_set: Option<TagSet>,
}

fn tag_name(tag_set: &Option<TagSet>) -> String {
    tag_set
        .as_ref()
        .map(|ts| {
            ts.items
                .iter()
                .find(|t| t.key.as_deref() == Some("Name"))
                .and_then(|t| t.value.clone())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

fn parse_instances(body: &str) -> Result<Vec<Instance>> {
    if let Ok(resp) = from_str::<DescribeInstancesResponse>(body) {
        let mut out = Vec::new();
        if let Some(rs) = resp.reservation_set {
            for res in rs.items {
                if let Some(iset) = res.instances_set {
                    for item in iset.items {
                        if let Some(id) = item.instance_id {
                            out.push(Instance {
                                instance_id: id,
                                name: tag_name(&item.tag_set),
                                state: item.instance_state.and_then(|s| s.name),
                                private_ip: item.private_ip,
                                public_ip: item.public_ip,
                            });
                        }
                    }
                }
            }
        }
        return Ok(out);
    }
    if let Ok(resp) = from_str::<RunInstancesResponse>(body) {
        let mut out = Vec::new();
        if let Some(iset) = resp.instances_set {
            for item in iset.items {
                if let Some(id) = item.instance_id {
                    out.push(Instance {
                        instance_id: id,
                        name: tag_name(&item.tag_set),
                        state: item.instance_state.and_then(|s| s.name),
                        private_ip: item.private_ip,
                        public_ip: item.public_ip,
                    });
                }
            }
        }
        return Ok(out);
    }
    Err(AwsError::Xml(format!(
        "unexpected EC2 instance response: {body}"
    )))
}

fn parse_volumes(body: &str) -> Result<Vec<Volume>> {
    if let Ok(resp) = from_str::<DescribeVolumesResponse>(body) {
        let mut out = Vec::new();
        if let Some(vs) = resp.volume_set {
            for item in vs.items {
                if let Some(id) = item.volume_id {
                    out.push(Volume {
                        volume_id: id,
                        name: tag_name(&item.tag_set),
                        size: item.size.and_then(|s| s.parse().ok()).unwrap_or(0),
                        volume_type: item.volume_type,
                    });
                }
            }
        }
        return Ok(out);
    }
    if let Ok(item) = from_str::<CreateVolumeResponse>(body) {
        if let Some(id) = item.volume_id {
            return Ok(vec![Volume {
                volume_id: id,
                name: tag_name(&item.tag_set),
                size: item.size.and_then(|s| s.parse().ok()).unwrap_or(0),
                volume_type: item.volume_type,
            }]);
        }
    }
    Err(AwsError::Xml(format!(
        "unexpected EC2 volume response: {body}"
    )))
}

#[derive(Debug, Deserialize)]
struct RunInstancesResponse {
    #[serde(rename = "instancesSet")]
    instances_set: Option<InstancesSet>,
}
