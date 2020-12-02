use std::collections::HashMap;
use std::sync::{Arc, Weak};

pub struct RecordStore<T>
where
    T: Entry,
{
    records: HashMap<String, Arc<T>>,
    lookup: HashMap<String, HashMap<String, Vec<Weak<T>>>>,
}

pub trait Entry {
    fn get_id(&self) -> &str;
    fn get_name(&self) -> &str;
    fn get_type(&self) -> &str;
}

impl<T> RecordStore<T>
where
    T: Entry,
{
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            lookup: HashMap::new(),
        }
    }

    pub fn add(&mut self, record: T) {
        let record = Arc::new(record);

        if self
            .records
            .insert(String::from(record.get_id()), Arc::clone(&record))
            .is_some()
        {
            self.remove_from_lookup(record.as_ref());
        }

        let by_type = self
            .lookup
            .entry(String::from(record.get_name()))
            .or_insert_with(HashMap::new);

        let record_list = by_type
            .entry(String::from(record.get_type()))
            .or_insert_with(Vec::new);

        record_list.push(Arc::downgrade(&record));
    }

    pub fn remove(&mut self, record: &T) {
        if self.records.remove(record.get_id()).is_some() {
            self.remove_from_lookup(record);
        }
    }

    pub fn get(&self, name: &str, record_type: &str) -> Vec<Arc<T>> {
        self.lookup
            .get(name)
            .and_then(|by_type| by_type.get(record_type))
            .map(|list| list.iter().filter_map(|r| r.upgrade()).collect())
            .unwrap_or_else(Vec::new)
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    fn remove_from_lookup(&mut self, record: &T) {
        if let Some(by_type) = self.lookup.get_mut(record.get_name()) {
            if let Some(list) = by_type.get_mut(record.get_type()) {
                list.retain(|v| match v.upgrade() {
                    Some(v) => v.get_id() == record.get_id(),
                    None => true,
                });

                if list.is_empty() {
                    by_type.remove(record.get_name());
                }

                if by_type.is_empty() {
                    self.lookup.remove(record.get_name());
                }
            }
        }
    }
}
