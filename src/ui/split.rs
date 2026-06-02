use ratatui::layout::Rect;
use crate::core::Position;
use crate::editor::cursor::Cursor;
use crate::editor::selection::Selection;

#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub id: usize,
    pub buffer_id: usize,
    pub cursor: Cursor,
    pub selection: Selection,
    pub scroll_offset: Position,
    pub viewport_height: usize,
    pub viewport_width: usize,
}

impl PaneInfo {
    pub fn new(id: usize, buffer_id: usize) -> Self {
        Self {
            id,
            buffer_id,
            cursor: Cursor::new(),
            selection: Selection::new(),
            scroll_offset: Position::zero(),
            viewport_height: 0,
            viewport_width: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SplitNode {
    Vertical { children: Vec<SplitNode>, ratios: Vec<f32> },
    Horizontal { children: Vec<SplitNode>, ratios: Vec<f32> },
    Pane(PaneInfo),
}

impl SplitNode {
    pub fn leaf_ids(&self) -> Vec<usize> {
        match self {
            SplitNode::Vertical { children, .. } | SplitNode::Horizontal { children, .. } => {
                children.iter().flat_map(|c| c.leaf_ids()).collect()
            }
            SplitNode::Pane(p) => vec![p.id],
        }
    }

    pub fn pane_mut(&mut self, id: usize) -> Option<&mut PaneInfo> {
        match self {
            SplitNode::Vertical { children, .. } | SplitNode::Horizontal { children, .. } => {
                for child in children {
                    if let Some(p) = child.pane_mut(id) {
                        return Some(p);
                    }
                }
                None
            }
            SplitNode::Pane(p) if p.id == id => Some(p),
            _ => None,
        }
    }

    pub fn count_panes(&self) -> usize {
        match self {
            SplitNode::Vertical { children, .. } | SplitNode::Horizontal { children, .. } => {
                children.iter().map(|c| c.count_panes()).sum()
            }
            SplitNode::Pane(_) => 1,
        }
    }

    pub fn replace_pane_with(&mut self, id: usize, replacement: &mut SplitNode) -> bool {
        match self {
            SplitNode::Vertical { children, .. } | SplitNode::Horizontal { children, .. } => {
                for child in children.iter_mut() {
                    if child.replace_pane_with(id, replacement) {
                        return true;
                    }
                }
                false
            }
            SplitNode::Pane(p) if p.id == id => {
                std::mem::swap(self, replacement);
                true
            }
            _ => false,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            SplitNode::Vertical { children, ratios } | SplitNode::Horizontal { children, ratios } => {
                if children.is_empty() {
                    return Err("split node has no children".to_string());
                }
                if children.len() != ratios.len() {
                    return Err(format!(
                        "child/ratio count mismatch: {} children, {} ratios",
                        children.len(),
                        ratios.len()
                    ));
                }
                let sum: f32 = ratios.iter().sum();
                if sum <= 0.0 {
                    return Err("ratios sum is not positive".to_string());
                }
                for (i, child) in children.iter().enumerate() {
                    child.validate().map_err(|e| format!("child[{}]: {}", i, e))?;
                }
                Ok(())
            }
            SplitNode::Pane(p) => {
                if p.id == usize::MAX {
                    return Err("pane has sentinel id usize::MAX".to_string());
                }
                Ok(())
            }
        }
    }
}

fn distribute_horizontal(area: Rect, ratios: &[f32]) -> Vec<Rect> {
    if ratios.is_empty() {
        return vec![area];
    }
    let total: f32 = ratios.iter().sum();
    if total == 0.0 {
        let each = (area.width / ratios.len() as u16).max(1);
        return ratios
            .iter()
            .enumerate()
            .map(|(i, _)| Rect::new(area.x + (each * i as u16), area.y, each, area.height))
            .collect();
    }
    let mut result = Vec::new();
    let mut offset = 0.0f32;
    for &ratio in ratios {
        let frac = ratio / total;
        let begin = (area.width as f32 * offset).round() as u16;
        let end = (area.width as f32 * (offset + frac)).round() as u16;
        let w = end.saturating_sub(begin).max(1);
        result.push(Rect::new(area.x + begin, area.y, w, area.height));
        offset += frac;
    }
    result
}

fn distribute_vertical(area: Rect, ratios: &[f32]) -> Vec<Rect> {
    if ratios.is_empty() {
        return vec![area];
    }
    let total: f32 = ratios.iter().sum();
    if total == 0.0 {
        let each = (area.height / ratios.len() as u16).max(1);
        return ratios
            .iter()
            .enumerate()
            .map(|(i, _)| Rect::new(area.x, area.y + (each * i as u16), area.width, each))
            .collect();
    }
    let mut result = Vec::new();
    let mut offset = 0.0f32;
    for &ratio in ratios {
        let frac = ratio / total;
        let begin = (area.height as f32 * offset).round() as u16;
        let end = (area.height as f32 * (offset + frac)).round() as u16;
        let h = end.saturating_sub(begin).max(1);
        result.push(Rect::new(area.x, area.y + begin, area.width, h));
        offset += frac;
    }
    result
}

pub fn layout_split_node(node: &SplitNode, area: Rect) -> Vec<(usize, Rect)> {
    match node {
        SplitNode::Horizontal { children, ratios } => {
            let ratios = normalize_ratios(ratios, children.len());
            let rects = distribute_horizontal(area, &ratios);
            let mut out = Vec::new();
            for (child, rect) in children.iter().zip(rects.iter()) {
                out.extend(layout_split_node(child, *rect));
            }
            out
        }
        SplitNode::Vertical { children, ratios } => {
            let ratios = normalize_ratios(ratios, children.len());
            let rects = distribute_vertical(area, &ratios);
            let mut out = Vec::new();
            for (child, rect) in children.iter().zip(rects.iter()) {
                out.extend(layout_split_node(child, *rect));
            }
            out
        }
        SplitNode::Pane(p) => vec![(p.id, area)],
    }
}

fn normalize_ratios(ratios: &[f32], target: usize) -> Vec<f32> {
    if target == 0 {
        return Vec::new();
    }
    if ratios.len() == target {
        return ratios.to_vec();
    }
    let mut out = ratios.to_vec();
    if out.len() < target {
        let avg = if out.is_empty() {
            1.0
        } else {
            out.iter().sum::<f32>() / out.len() as f32
        };
        while out.len() < target {
            out.push(avg);
        }
    } else {
        out.truncate(target);
    }
    out
}

pub struct SplitManager {
    pub root: SplitNode,
    pub active_pane_id: usize,
    next_id: usize,
}

impl SplitManager {
    pub fn new(initial_buffer_id: usize) -> Self {
        let id = 0;
        Self {
            root: SplitNode::Pane(PaneInfo::new(id, initial_buffer_id)),
            active_pane_id: id,
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn active_pane(&mut self) -> Option<&mut PaneInfo> {
        self.root.pane_mut(self.active_pane_id)
    }

    pub fn pane_by_id(&mut self, id: usize) -> Option<&mut PaneInfo> {
        self.root.pane_mut(id)
    }

    pub fn active_buffer_id(&self) -> usize {
        let mut buf_id = 0;
        self.for_each_pane(&mut |p| {
            if p.id == self.active_pane_id {
                buf_id = p.buffer_id;
            }
        });
        buf_id
    }

    pub fn for_each_pane<F: FnMut(&PaneInfo)>(&self, f: &mut F) {
        self.for_each_node(&self.root, f);
    }

    fn for_each_node<F: FnMut(&PaneInfo)>(&self, node: &SplitNode, f: &mut F) {
        match node {
            SplitNode::Vertical { children, .. } | SplitNode::Horizontal { children, .. } => {
                for child in children {
                    self.for_each_node(child, f);
                }
            }
            SplitNode::Pane(p) => f(p),
        }
    }

    pub fn split_horizontal(&mut self, buffer_id: usize) {
        let current_id = self.active_pane_id;
        let new_id = self.alloc_id();
        let old_state = self.root.pane_mut(current_id).map(|p| p.clone());
        let old_state = match old_state {
            Some(s) => s,
            None => return,
        };
        let mut replacement = SplitNode::Horizontal {
            children: vec![
                SplitNode::Pane(PaneInfo { id: current_id, ..old_state.clone() }),
                SplitNode::Pane(PaneInfo::new(new_id, buffer_id)),
            ],
            ratios: vec![0.5, 0.5],
        };
        self.root.replace_pane_with(current_id, &mut replacement);
        self.active_pane_id = new_id;
    }

    pub fn split_vertical(&mut self, buffer_id: usize) {
        let current_id = self.active_pane_id;
        let new_id = self.alloc_id();
        let old_state = self.root.pane_mut(current_id).map(|p| p.clone());
        let old_state = match old_state {
            Some(s) => s,
            None => return,
        };
        let mut replacement = SplitNode::Vertical {
            children: vec![
                SplitNode::Pane(PaneInfo { id: current_id, ..old_state.clone() }),
                SplitNode::Pane(PaneInfo::new(new_id, buffer_id)),
            ],
            ratios: vec![0.5, 0.5],
        };
        self.root.replace_pane_with(current_id, &mut replacement);
        self.active_pane_id = new_id;
    }

    pub fn close_pane(&mut self, id: usize) -> Option<usize> {
        if self.root.count_panes() <= 1 {
            return None;
        }
        let mut buffer_id = None;
        if let SplitNode::Pane(p) = &self.root {
            if p.id == id {
                return None;
            }
        }
        close_in_node(&mut self.root, id, &mut buffer_id);
        let leaves = self.root.leaf_ids();
        if !leaves.contains(&self.active_pane_id) {
            if let Some(&first) = leaves.first() {
                self.active_pane_id = first;
            }
        }
        buffer_id
    }

    pub fn focus_next(&mut self) {
        let ids = self.root.leaf_ids();
        if ids.len() <= 1 {
            return;
        }
        let pos = ids.iter().position(|&id| id == self.active_pane_id).unwrap_or(0);
        self.active_pane_id = ids[(pos + 1) % ids.len()];
    }

    pub fn focus_prev(&mut self) {
        let ids = self.root.leaf_ids();
        if ids.len() <= 1 {
            return;
        }
        let pos = ids.iter().position(|&id| id == self.active_pane_id).unwrap_or(0);
        self.active_pane_id = ids[(pos + ids.len() - 1) % ids.len()];
    }

    pub fn panes_count(&self) -> usize {
        self.root.count_panes()
    }
}

fn close_in_node(node: &mut SplitNode, id: usize, out_buffer: &mut Option<usize>) {
    match node {
        SplitNode::Vertical { children, ratios } | SplitNode::Horizontal { children, ratios } => {
            let mut i = 0;
            while i < children.len() {
                if let SplitNode::Pane(p) = &children[i] {
                    if p.id == id {
                        *out_buffer = Some(p.buffer_id);
                        children.remove(i);
                        ratios.remove(i);
                        if children.len() == 1 {
                            let sole = children.remove(0);
                            *node = sole;
                        }
                        return;
                    }
                }
                i += 1;
            }
            for child in children.iter_mut() {
                close_in_node(child, id, out_buffer);
                if out_buffer.is_some() {
                    if children.len() == 1 {
                        let sole = children.remove(0);
                        *node = sole;
                    }
                    return;
                }
            }
            if !children.is_empty() {
                let sum: f32 = ratios.iter().sum();
                if sum > 0.0 {
                    for r in ratios.iter_mut() {
                        *r /= sum;
                    }
                }
            }
        }
        SplitNode::Pane(_) => {}
    }
}
