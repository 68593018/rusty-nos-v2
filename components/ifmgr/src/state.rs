use std::collections::HashMap;
use nos_common::data::interface::InterfaceEntry;

#[derive(Debug, Default)]
pub struct IfMgrState {
    // 接口名 -> 接口信息 (e.g., "eth0" -> Entry)
    pub interfaces: HashMap<String, InterfaceEntry>,
}

impl IfMgrState {
    pub fn update(&mut self, entry: InterfaceEntry) {
        self.interfaces.insert(entry.name.clone(), entry);
    }
}