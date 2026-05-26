
use lsp_types::*;

pub struct GotoHandler;

#[derive(Debug, Clone)]
pub struct JumpTarget {
    pub uri: lsp_types::Url,
    pub range: Range,
    pub origin_uri: Option<lsp_types::Url>,
    pub origin_range: Option<Range>,
}

impl GotoHandler {
    pub fn process_definition_response(
        result: serde_json::Value,
    ) -> Vec<LocationLink> {
        if let Ok(single) = serde_json::from_value::<Location>(result.clone()) {
            return vec![convert_location_to_link(single)];
        }
        if let Ok(multiple) = serde_json::from_value::<Vec<Location>>(result.clone()) {
            return multiple.into_iter()
                .map(convert_location_to_link)
                .collect();
        }
        if let Ok(links) = serde_json::from_value::<Vec<LocationLink>>(result) {
            return links;
        }
        tracing::warn!("Failed to parse definition response");
        vec![]
    }

    pub fn process_references_response(
        result: serde_json::Value,
    ) -> Vec<Location> {
        if let Ok(single) = serde_json::from_value::<Location>(result.clone()) {
            return vec![single];
        }
        if let Ok(multiple) = serde_json::from_value::<Vec<Location>>(result) {
            return multiple;
        }
        tracing::warn!("Failed to parse references response");
        vec![]
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
