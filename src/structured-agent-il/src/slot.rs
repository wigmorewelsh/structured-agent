#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    ReturnSlot,
    ValueParam,
    Local,
    Temp,
}

#[derive(Clone, Debug)]
pub struct SlotInfo {
    pub slot: Slot,
    pub kind: SlotKind,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct SlotTable {
    slots: Vec<SlotInfo>,
}

impl SlotTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, kind: SlotKind, name: impl Into<String>) -> Slot {
        let slot = Slot(self.slots.len() as u32);
        self.slots.push(SlotInfo { slot, kind, name: name.into() });
        slot
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn get(&self, slot: Slot) -> Option<&SlotInfo> {
        self.slots.get(slot.0 as usize)
    }

    pub fn iter(&self) -> impl Iterator<Item = &SlotInfo> {
        self.slots.iter()
    }
}
