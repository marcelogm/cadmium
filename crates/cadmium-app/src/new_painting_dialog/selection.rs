use cadmium_engine::pigments::{MAX_SLOTS, PigmentDb};

#[derive(Clone, Debug, PartialEq)]
pub struct PigmentSelection {
    available: Vec<&'static str>,
    chosen: Vec<&'static str>,
}

impl PigmentSelection {
    pub fn new(db: &PigmentDb, initial: &[String]) -> Self {
        let mut selection = PigmentSelection {
            available: db.all().iter().map(|pigment| pigment.id).collect(),
            chosen: Vec::new(),
        };
        for id in initial {
            selection.set(id, true);
        }
        selection
    }

    pub fn contains(&self, id: &str) -> bool {
        self.chosen.contains(&id)
    }

    pub fn count(&self) -> usize {
        self.chosen.len()
    }

    pub fn set(&mut self, id: &str, chosen: bool) -> bool {
        let Some(&id) = self.available.iter().find(|known| **known == id) else {
            return !chosen;
        };
        match (chosen, self.contains(id)) {
            (true, true) | (false, false) => true,
            (false, true) => {
                self.chosen.retain(|kept| *kept != id);
                true
            }
            (true, false) if self.count() < MAX_SLOTS => {
                self.chosen.push(id);
                true
            }
            (true, false) => false,
        }
    }

    pub fn apply_preset(&mut self, pigments: &[&str]) {
        self.chosen.clear();
        for id in pigments {
            self.set(id, true);
        }
    }

    pub fn slots(&self) -> Vec<String> {
        self.available
            .iter()
            .filter(|id| self.contains(id))
            .map(|id| id.to_string())
            .collect()
    }
}

pub fn counter_text(count: usize) -> String {
    format!("{count} de {MAX_SLOTS}")
}

#[cfg(test)]
mod tests;
