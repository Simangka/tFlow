
use lsp_types::*;

pub struct GotoHandler;

#[derive(Debug, Clone)]
pub struct JumpTarget {
    pub uri: lsp_types::Url,
    pub range: Range,
    pub origin_uri: Option<lsp_types::Url>,
    pub origin_range: Option<Range>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum DefinitionResponse {
    Single(Location),
    Multiple(Vec<Location>),
    Links(Vec<LocationLink>),
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ReferencesResponse {
    Single(Location),
    Multiple(Vec<Location>),
}

impl GotoHandler {
    pub fn process_definition_response(
        result: serde_json::Value,
    ) -> Vec<LocationLink> {
        match serde_json::from_value::<DefinitionResponse>(result) {
            Ok(DefinitionResponse::Single(loc)) => vec![convert_location_to_link(loc)],
            Ok(DefinitionResponse::Multiple(locs)) => locs
                .into_iter()
                .map(convert_location_to_link)
                .collect(),
            Ok(DefinitionResponse::Links(links)) => links,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse definition response");
                vec![]
            }
        }
    }

    pub fn process_references_response(
        result: serde_json::Value,
    ) -> Vec<Location> {
        match serde_json::from_value::<ReferencesResponse>(result) {
            Ok(ReferencesResponse::Single(loc)) => vec![loc],
            Ok(ReferencesResponse::Multiple(locs)) => locs,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse references response");
                vec![]
            }
        }
    }

    pub fn jump_target_to_location(target: &JumpTarget) -> Location {
        Location {
            uri: target.uri.clone(),
            range: target.range,
        }
    }
}

fn convert_location_to_link(loc: Location) -> LocationLink {
    LocationLink {
        origin_selection_range: None,
        target_uri: loc.uri,
        target_range: loc.range,
        target_selection_range: loc.range,
    }
}
