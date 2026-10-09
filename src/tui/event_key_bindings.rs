use crokey::KeyCombination;

#[derive(Debug, Clone)]
pub struct EventKeyBinding {
    pub keys: Vec<KeyCombination>,
    pub event_name: String,
}

impl EventKeyBinding {
    pub fn new(keys: Vec<KeyCombination>, event_name: &str, _short_name: Option<&str>) -> EventKeyBinding {
        EventKeyBinding {
            keys,
            event_name: String::from(event_name),

        }
    }

}
