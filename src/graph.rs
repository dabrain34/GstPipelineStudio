#[derive(Debug, Clone, Default)]
pub struct Element {
    pub name: String,
    pub position: (f64, f64),
    pub size: (f64, f64),
}

#[derive(Debug)]
pub struct Graph {
    elements: Vec<Element>,
    last_x_position: u32,
}

impl Default for Graph {
    fn default() -> Graph {
        Graph {
            elements: vec![],
            last_x_position: 0,
        }
    }
}

impl Graph {
    pub fn elements(&mut self) -> &Vec<Element> {
        &self.elements
    }

    pub fn add_element(&mut self, element: Element) {
        self.elements.push(element);
    }

    pub fn remove_element(&mut self, name: &str) {
        let index = self.elements.iter().position(|x| x.name == name).unwrap();
        self.elements.remove(index);
    }
}
