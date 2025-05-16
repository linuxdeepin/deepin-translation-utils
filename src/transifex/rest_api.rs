// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// SPDX-License-Identifier: MIT

// Transifex OpenAPI doc: https://transifex.github.io/openapi/

use directories::BaseDirs;
use serde::Deserialize;
use thiserror::Error as TeError;

use super::{tx_config_file::{load_transifexrc_file, LoadTxConfigError}, yaml_file::TxResourceLookupEntry};

pub struct TransifexRestApi {
    rest_hostname: String,
    token: String,
}

#[derive(TeError, Debug)]
pub enum TransifexRestApiError {
    #[error("Error making request: {0}")]
    Ureq(#[from] ureq::Error),
    #[error("Error parsing response: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Deserialize, Clone, Debug)]
pub struct TransifexDataAttributes {
    pub categories: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TransifexData {
    /// This field should follow format:
    /// `o:organization_slug:p:project_slug:r:resource_slug`
    pub id: String,
    pub attributes: TransifexDataAttributes,
}

impl TransifexData {
    pub fn parse_linked_resource_category(&self) -> Option<TxResourceLookupEntry> {
        let binding = self.attributes.categories.clone()?;
        let category = binding.iter()
            .find(|&c| c.starts_with("github#repository:"))?;

        let re = regex::Regex::new(r"^github#repository:(?P<organization>[^/]+)/(?P<repository>[^#]+)#branch:(?P<branch>[^#]+)#path:(?P<path>.+)$").unwrap();
        let captures = re.captures(category)?;
        let organization = captures.name("organization")?.as_str();
        let repository = captures.name("repository")?.as_str();
        let branch = captures.name("branch")?.as_str();
        let path = captures.name("path")?.as_str();
        Some(TxResourceLookupEntry {
            repository: format!("{organization}/{repository}"),
            branch: branch.to_string(),
            resource: path.to_string(),
            transifex_resource_id: self.id.to_string(),
        })
    }
}

#[derive(Deserialize, Debug)]
pub struct TransifexPaginationResponse<T> {
    pub data: Vec<T>,
    links: TransifexPaginationLinks,
}

pub trait Paginated {
    type T;
    fn next_page_url(&self) -> Option<&str>;
    fn items(self) -> Vec<Self::T>;
}

impl<T> Paginated for TransifexPaginationResponse<T> {
    type T = T;
    fn next_page_url(&self) -> Option<&str> {
        self.links.next.as_deref()
    }
    fn items(self) -> Vec<Self::T> {
        self.data
    }
}

#[derive(Deserialize, Debug)]
struct TransifexPaginationLinks {
    next: Option<String>,
    #[allow(dead_code)]
    previous: Option<String>,
    #[allow(dead_code)]
    self_attr: Option<String>,
}

impl TransifexRestApi {
    pub fn new(rest_hostname: &str, token: &str) -> Self {
        Self {
            rest_hostname: rest_hostname.to_string(),
            token: token.to_string(),
        }
    }

    pub fn new_from_transifexrc() -> Result<Self, LoadTxConfigError> {
        let xdg_dirs = BaseDirs::new().expect("Not able to get xdg base directories");
        let transifexrc_file = xdg_dirs.home_dir().join(".transifexrc");
        let transifexrc = load_transifexrc_file(&transifexrc_file)?;
        Ok(TransifexRestApi::new(&transifexrc.rest_hostname, &transifexrc.token))
    }
    
    pub fn fetch_paginated<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<Vec<T>, TransifexRestApiError> {
        let mut all_items = Vec::<T>::new();
        let mut next_page_url = Some(self.rest_hostname.clone() + url);
        while let Some(url) = next_page_url {
            let mut resp = ureq::get(&url)
                .header("Authorization", &format!("Bearer {}", self.token))
                .call()?;
            let resp_text = resp.body_mut().read_to_string()?;
            let resp_json = serde_json::from_str::<TransifexPaginationResponse<T>>(&resp_text)?;
            let next_url = resp_json.next_page_url().map(|s| s.to_string());
            all_items.extend(resp_json.items());
            next_page_url = next_url;
        }
        Ok(all_items)
    }

    pub fn get_all_projects(&self, organization_slug: &str) -> Result<Vec<TransifexData>, TransifexRestApiError> {
        let url = format!("/projects?filter[organization]=o:{}", organization_slug);
        self.fetch_paginated::<TransifexData>(&url)
    }

    pub fn get_all_linked_resources(&self, organization_slug: &str, project_slug: &str) -> Result<Vec<TransifexData>, TransifexRestApiError> {
        let url = format!("/resources?filter[project]=o:{}:p:{}", organization_slug, project_slug);
        let resources = self.fetch_paginated::<TransifexData>(&url)?;
        // linked resources are those with category attribute and match the following pattern:
        // github#repository:organization/repository#branch:branch#path:path/to/file
        let linked_resources = resources.into_iter().filter(|resource| {
            resource.attributes.categories.as_ref().map_or(false, |categories| {
                categories.iter().any(|entry| entry.starts_with("github#repository:"))
            })
        }).collect();
        Ok(linked_resources)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn tst_parse_resources_response() {
        let resp_text = r#"{
    "data": [
        {
            "id": "o:linuxdeepin:p:deepin-home:r:bad354a0c370deff052c13b687289331",
            "type": "resources",
            "attributes": {
                "slug": "bad354a0c370deff052c13b687289331",
                "name": "translations/deepin-home.ts (master)",
                "priority": "high",
                "i18n_type": "QT",
                "accept_translations": true,
                "categories": [
                    "github#repository:linuxdeepin/deepin-home#branch:master#path:translations/deepin-home.ts"
                ]
            }
        },
        {
            "id": "o:linuxdeepin:p:deepin-home:r:dummy-not-linked-resource",
            "type": "resources",
            "attributes": {
                "slug": "dummy-not-linked-resource",
                "name": "dummy-not-linked-resource",
                "priority": "high",
                "i18n_type": "QT"
            }
        }
    ],
    "links": {
        "self": "https://rest.api.transifex.com/resources?filter[project]=o:linuxdeepin:p:deepin-home",
        "next": null,
        "previous": null
    }
}"#;
        let resp_json: TransifexPaginationResponse<TransifexData> = serde_json::from_str(resp_text).unwrap();
        println!("{:?}", resp_json);
    }
}