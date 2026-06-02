//! OVHcloud API v2 **iam** bindings (`/v2/iam`).
//!
//! Generated from the official schema; do not edit by hand.

#![allow(unused_imports, unused_variables)]

use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

pub use crate::iam::{ResourceMetadata, ResourceState};

/// `dbaas.logs.LogKind`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogKind {
    pub additional_returned_fields: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub display_name: Option<String>,
    pub kind_id: Option<String>,
    pub name: Option<String>,
    pub updated_at: Option<String>,
}

/// `dbaas.logs.LogSubscriptionResource`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogSubscriptionResource {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `dbaas.logs.LogSubscription`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogSubscription {
    pub created_at: Option<String>,
    pub kind: Option<String>,
    pub resource: Option<LogsLogSubscriptionResource>,
    pub service_name: Option<String>,
    pub stream_id: Option<String>,
    pub subscription_id: Option<String>,
    pub updated_at: Option<String>,
}

/// `dbaas.logs.LogSubscriptionCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogSubscriptionCreation {
    pub kind: String,
    pub stream_id: String,
}

/// `dbaas.logs.LogSubscriptionResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogSubscriptionResponse {
    pub operation_id: Option<String>,
    pub service_name: Option<String>,
}

/// `dbaas.logs.LogUrlCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogUrlCreation {
    pub kind: String,
}

/// `dbaas.logs.TemporaryLogsLink`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsTemporaryLogsLink {
    pub expiration_date: Option<String>,
    pub url: Option<String>,
}

/// `iam.AuthorizeBatchRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeBatchRequest {
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub resource_urns: Vec<String>,
}

/// `iam.AuthorizeBatchResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeBatchResponse {
    pub authorized_actions: Option<Vec<String>>,
    pub resource_urn: Option<String>,
    pub unauthorized_actions: Option<Vec<String>>,
}

/// `iam.AuthorizeRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeRequest {
    #[serde(default)]
    pub actions: Vec<String>,
}

/// `iam.AuthorizeResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeResponse {
    pub authorized_actions: Option<Vec<String>>,
    pub unauthorized_actions: Option<Vec<String>>,
}

/// `iam.policy.Action`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyAction {
    pub action: String,
}

/// `iam.policy.Permissions`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPermissions {
    pub allow: Option<Vec<PolicyAction>>,
    pub deny: Option<Vec<PolicyAction>>,
    pub except: Option<Vec<PolicyAction>>,
}

/// `iam.PermissionsGroup`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsGroup {
    pub created_at: Option<String>,
    pub description: String,
    pub id: Option<String>,
    pub name: String,
    pub owner: Option<String>,
    pub permissions: PolicyPermissions,
    pub updated_at: Option<String>,
    pub urn: Option<String>,
}

/// `iam.group.Resource`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupResource {
    pub display_name: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub owner: Option<String>,
    #[serde(default)]
    pub tags: serde_json::Value,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub urn: Option<String>,
}

/// `iam.group.Creation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupCreation {
    pub name: String,
    pub resources: Option<Vec<GroupResource>>,
}

/// `iam.group.Response`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupResponse {
    pub created_at: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub owner: Option<String>,
    pub read_only: Option<bool>,
    pub resources: Option<Vec<GroupResource>>,
    pub updated_at: Option<String>,
    pub urn: Option<String>,
}

/// `iam.group.Update`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupUpdate {
    pub name: String,
    #[serde(default)]
    pub resources: Vec<GroupResource>,
}

/// `iam.policy.Condition.OperatorEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IamPolicyConditionOperator {
    #[serde(rename = "AND")]
    And,
    #[serde(rename = "MATCH")]
    Match,
    #[serde(rename = "NOT")]
    Not,
    #[serde(rename = "OR")]
    Or,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `iam.policy.Condition`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyCondition {
    pub conditions: Option<Vec<PolicyCondition>>,
    pub operator: IamPolicyConditionOperator,
    #[serde(default)]
    pub values: serde_json::Value,
}

/// `iam.policy.PermissionsGroup`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPermissionsGroup {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub permissions: Option<PolicyPermissions>,
    pub urn: Option<String>,
}

/// `iam.policy.Group`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyGroup {
    pub id: Option<String>,
    pub name: Option<String>,
    pub read_only: Option<bool>,
}

/// `iam.policy.SingleResource`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySingleResource {
    pub display_name: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub owner: Option<String>,
    #[serde(default)]
    pub tags: serde_json::Value,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `iam.policy.Resource`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyResource {
    pub group: Option<PolicyGroup>,
    pub resource: Option<PolicySingleResource>,
    pub urn: String,
}

/// `iam.policy.Creation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyCreation {
    pub conditions: Option<PolicyCondition>,
    pub description: Option<String>,
    pub expired_at: Option<String>,
    #[serde(default)]
    pub identities: Vec<String>,
    pub name: String,
    pub permissions: PolicyPermissions,
    pub permissions_groups: Option<Vec<PolicyPermissionsGroup>>,
    #[serde(default)]
    pub resources: Vec<PolicyResource>,
}

/// `iam.policy.Response`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyResponse {
    pub conditions: Option<PolicyCondition>,
    pub created_at: Option<String>,
    pub description: Option<String>,
    pub expired_at: Option<String>,
    pub id: Option<String>,
    pub identities: Option<Vec<String>>,
    pub name: Option<String>,
    pub owner: Option<String>,
    pub permissions: Option<PolicyPermissions>,
    pub permissions_groups: Option<Vec<PolicyPermissionsGroup>>,
    pub read_only: Option<bool>,
    pub resources: Option<Vec<PolicyResource>>,
    pub updated_at: Option<String>,
}

/// `iam.policy.Update`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyUpdate {
    pub conditions: Option<PolicyCondition>,
    pub description: Option<String>,
    pub expired_at: Option<String>,
    #[serde(default)]
    pub identities: Vec<String>,
    pub name: String,
    pub permissions: PolicyPermissions,
    pub permissions_groups: Option<Vec<PolicyPermissionsGroup>>,
    #[serde(default)]
    pub resources: Vec<PolicyResource>,
}

/// `iam.policy.conditions.CombinatorOperatorContract`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyConditionsCombinatorOperatorContract {
    pub conditions: Option<bool>,
    pub max_conditions: Option<String>,
    pub min_conditions: Option<String>,
    pub values: Option<bool>,
}

/// `iam.policy.conditions.CombinatorOperator`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyConditionsCombinatorOperator {
    pub contract: Option<PolicyConditionsCombinatorOperatorContract>,
    pub description: Option<String>,
    pub operator_name: Option<IamPolicyConditionOperator>,
}

/// `iam.policy.conditions.ComparatorOperatorFunctionDefinition`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyConditionsComparatorOperatorFunctionDefinition {
    pub function_name: Option<String>,
    pub max_args: Option<String>,
    pub min_args: Option<String>,
}

/// `iam.policy.conditions.ComparatorOperator`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyConditionsComparatorOperator {
    pub description: Option<String>,
    pub example: Option<String>,
    pub function_definitions: Option<Vec<PolicyConditionsComparatorOperatorFunctionDefinition>>,
    pub key: Option<String>,
    pub matching_values_format: Option<String>,
    pub parameter_type: Option<String>,
    pub template: Option<String>,
}

/// `iam.policy.conditions.Schema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyConditionsSchema {
    pub combinator: Option<Vec<PolicyConditionsCombinatorOperator>>,
    pub comparator: Option<Vec<PolicyConditionsComparatorOperator>>,
}

/// `iam.reference.Action`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceAction {
    pub action: Option<String>,
    pub categories: Option<Vec<String>>,
    pub description: Option<String>,
    pub has_query_parameters: Option<bool>,
    pub resource_type: Option<String>,
}

/// `iam.resource.AddTag`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceAddTag {
    pub key: String,
    pub value: String,
}

/// `iam.resource.Resource`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceResource {
    pub display_name: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub owner: Option<String>,
    #[serde(default)]
    pub tags: serde_json::Value,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub urn: Option<String>,
}

/// `iam.resource.TagFilter.OperatorEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IamResourceTagFilterOperator {
    #[serde(rename = "EQ")]
    Eq,
    #[serde(rename = "EXISTS")]
    Exists,
    #[serde(rename = "ILIKE")]
    Ilike,
    #[serde(rename = "LIKE")]
    Like,
    #[serde(rename = "NEQ")]
    Neq,
    #[serde(rename = "NEXISTS")]
    Nexists,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `iam.resource.TagFilter`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTagFilter {
    pub operator: Option<IamResourceTagFilterOperator>,
    pub value: Option<String>,
}

impl OvhClient {
    /// `POST /iam/authorization/check` — Validate your authorizations on given resources
    pub async fn iam_authorization_check_post(
        &self,
        body: &AuthorizeBatchRequest,
    ) -> Result<Vec<AuthorizeBatchResponse>> {
        self.post_v2(
            "/iam/authorization/check",
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /iam/log/kind` — List available log kinds
    pub async fn iam_log_kinds(&self, page: &PageParams) -> Result<Vec<String>> {
        self.get_page("/iam/log/kind", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /iam/log/kind/{name}` — Get a log kind
    pub async fn iam_log_kind(&self, name: &str) -> Result<LogsLogKind> {
        self.get(&format!("/iam/log/kind/{}", percent_encode(name)))
            .await
    }

    /// `GET /iam/log/subscription` — List subscription IDs for a cluster
    pub async fn iam_log_subscriptions(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<String>> {
        self.get_page(
            &Self::append_query("/iam/log/subscription", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /iam/log/subscription` — Create a subscription from logs to a pre-existing LDP stream
    pub async fn iam_log_subscription_post(
        &self,
        body: &LogsLogSubscriptionCreation,
    ) -> Result<LogsLogSubscriptionResponse> {
        self.post_v2("/iam/log/subscription", body, V2RequestOptions::default())
            .await
    }

    /// `DELETE /iam/log/subscription/{subscriptionId}` — Delete a subscription
    pub async fn iam_log_subscription_delete(
        &self,
        subscription_id: &str,
    ) -> Result<LogsLogSubscriptionResponse> {
        self.delete_json(&format!(
            "/iam/log/subscription/{}",
            percent_encode(subscription_id)
        ))
        .await
    }

    /// `GET /iam/log/subscription/{subscriptionId}` — Get subscription details
    pub async fn iam_log_subscription(&self, subscription_id: &str) -> Result<LogsLogSubscription> {
        self.get(&format!(
            "/iam/log/subscription/{}",
            percent_encode(subscription_id)
        ))
        .await
    }

    /// `POST /iam/log/url` — Generate a temporary URL to retrieve logs
    pub async fn iam_log_url_post(
        &self,
        body: &LogsLogUrlCreation,
    ) -> Result<LogsTemporaryLogsLink> {
        self.post_v2("/iam/log/url", body, V2RequestOptions::default())
            .await
    }

    /// `GET /iam/permissionsGroup` — Retrieve all permissions groups
    pub async fn iam_permissions_groups(&self, page: &PageParams) -> Result<Vec<PermissionsGroup>> {
        self.get_page("/iam/permissionsGroup", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `POST /iam/permissionsGroup` — Create a permissions group
    pub async fn iam_permissions_group_post(
        &self,
        body: &PermissionsGroup,
    ) -> Result<PermissionsGroup> {
        self.post_v2("/iam/permissionsGroup", body, V2RequestOptions::default())
            .await
    }

    /// `DELETE /iam/permissionsGroup/{permissionsGroupURN}` — Delete the given permissions group
    pub async fn iam_permissions_group_delete(&self, permissions_group_urn: &str) -> Result<()> {
        self.delete(&format!(
            "/iam/permissionsGroup/{}",
            percent_encode(permissions_group_urn)
        ))
        .await
    }

    /// `GET /iam/permissionsGroup/{permissionsGroupURN}` — Retrieve the given permissions group
    pub async fn iam_permissions_group(
        &self,
        permissions_group_urn: &str,
    ) -> Result<PermissionsGroup> {
        self.get(&format!(
            "/iam/permissionsGroup/{}",
            percent_encode(permissions_group_urn)
        ))
        .await
    }

    /// `PUT /iam/permissionsGroup/{permissionsGroupURN}` — Update a permissions group
    pub async fn iam_permissions_group_put(
        &self,
        permissions_group_urn: &str,
        body: &PermissionsGroup,
    ) -> Result<PermissionsGroup> {
        self.put_json(
            &format!(
                "/iam/permissionsGroup/{}",
                percent_encode(permissions_group_urn)
            ),
            body,
        )
        .await
    }

    /// `GET /iam/policy` — Retrieve all policies
    pub async fn iam_policys(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<PolicyResponse>> {
        self.get_page(&Self::append_query("/iam/policy", query), &[], page)
            .await
            .map(|p| p.items)
    }

    /// `POST /iam/policy` — Create a new policy
    pub async fn iam_policy_post(&self, body: &PolicyCreation) -> Result<PolicyResponse> {
        self.post_v2("/iam/policy", body, V2RequestOptions::default())
            .await
    }

    /// `DELETE /iam/policy/{policyId}` — Delete the given policy
    pub async fn iam_policy_delete(&self, policy_id: &str) -> Result<()> {
        self.delete(&format!("/iam/policy/{}", percent_encode(policy_id)))
            .await
    }

    /// `GET /iam/policy/{policyId}` — Retrieve the given policy
    pub async fn iam_policy(
        &self,
        policy_id: &str,
        query: &[(&str, &str)],
    ) -> Result<PolicyResponse> {
        self.get(&Self::append_query(
            &format!("/iam/policy/{}", percent_encode(policy_id)),
            query,
        ))
        .await
    }

    /// `PUT /iam/policy/{policyId}` — Update an existing policy
    pub async fn iam_policy_put(
        &self,
        policy_id: &str,
        body: &PolicyUpdate,
    ) -> Result<PolicyResponse> {
        self.put_json(&format!("/iam/policy/{}", percent_encode(policy_id)), body)
            .await
    }

    /// `GET /iam/reference/action` — Retrieve all actions
    pub async fn iam_reference_actions(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ReferenceAction>> {
        self.get_page(
            &Self::append_query("/iam/reference/action", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /iam/reference/resource/type` — Retrieve all resource types
    pub async fn iam_reference_resource_types(&self, page: &PageParams) -> Result<Vec<String>> {
        self.get_page("/iam/reference/resource/type", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /iam/resource` — List all resources
    pub async fn iam_resources(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ResourceResource>> {
        self.get_page(&Self::append_query("/iam/resource", query), &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /iam/resource/{resourceURN}` — Retrieve a resource
    pub async fn iam_resource(&self, resource_urn: &str) -> Result<ResourceResource> {
        self.get(&format!("/iam/resource/{}", percent_encode(resource_urn)))
            .await
    }

    /// `PUT /iam/resource/{resourceURN}` — Update an existing resource
    pub async fn iam_resource_put(
        &self,
        resource_urn: &str,
        body: &ResourceResource,
    ) -> Result<ResourceResource> {
        self.put_json(
            &format!("/iam/resource/{}", percent_encode(resource_urn)),
            body,
        )
        .await
    }

    /// `POST /iam/resource/{resourceURN}/authorization/check` — Validate authorizations on a given resource
    pub async fn iam_resource_authorization_check_post(
        &self,
        resource_urn: &str,
        body: &AuthorizeRequest,
    ) -> Result<AuthorizeResponse> {
        self.post_v2(
            &format!(
                "/iam/resource/{}/authorization/check",
                percent_encode(resource_urn)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /iam/resource/{resourceURN}/tag/{key}` — Remove a tag from a resource
    pub async fn iam_resource_tag_delete(&self, key: &str, resource_urn: &str) -> Result<()> {
        self.delete(&format!(
            "/iam/resource/{}/tag/{}",
            percent_encode(resource_urn),
            percent_encode(key)
        ))
        .await
    }

    /// `POST /iam/resource/{resourceURN}/tag` — Add a tag to a resource
    pub async fn iam_resource_tag_post(
        &self,
        resource_urn: &str,
        body: &ResourceAddTag,
    ) -> Result<()> {
        self.post_void(
            &format!("/iam/resource/{}/tag", percent_encode(resource_urn)),
            body,
        )
        .await
    }

    /// `GET /iam/resourceGroup` — Retrieve all resource groups
    pub async fn iam_resource_groups(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<GroupResponse>> {
        self.get_page(&Self::append_query("/iam/resourceGroup", query), &[], page)
            .await
            .map(|p| p.items)
    }

    /// `POST /iam/resourceGroup` — Create a new resource group
    pub async fn iam_resource_group_post(&self, body: &GroupCreation) -> Result<GroupResponse> {
        self.post_v2("/iam/resourceGroup", body, V2RequestOptions::default())
            .await
    }

    /// `DELETE /iam/resourceGroup/{groupId}` — Delete the given resource group
    pub async fn iam_resource_group_delete(&self, group_id: &str) -> Result<()> {
        self.delete(&format!("/iam/resourceGroup/{}", percent_encode(group_id)))
            .await
    }

    /// `GET /iam/resourceGroup/{groupId}` — Retrieve the given resource group
    pub async fn iam_resource_group(
        &self,
        group_id: &str,
        query: &[(&str, &str)],
    ) -> Result<GroupResponse> {
        self.get(&Self::append_query(
            &format!("/iam/resourceGroup/{}", percent_encode(group_id)),
            query,
        ))
        .await
    }

    /// `PUT /iam/resourceGroup/{groupId}` — Update an existing resource group
    pub async fn iam_resource_group_put(
        &self,
        group_id: &str,
        body: &GroupUpdate,
    ) -> Result<GroupResponse> {
        self.put_json(
            &format!("/iam/resourceGroup/{}", percent_encode(group_id)),
            body,
        )
        .await
    }
}
