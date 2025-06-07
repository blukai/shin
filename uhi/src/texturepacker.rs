#[derive(Debug)]
struct Node<T> {
    value: T,
    parent: Option<usize>,
    first_child: Option<usize>,
    next_sibling: Option<usize>,
}

impl<T> Node<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            parent: None,
            first_child: None,
            next_sibling: None,
        }
    }
}

/// a stupid n-way tree
#[derive(Debug)]
struct Tree<T> {
    nodes: Vec<Node<T>>,
    root_index: usize,
}

impl<T> Tree<T> {
    fn new(value: T) -> Self {
        Self {
            nodes: vec![Node::new(value)],
            root_index: 0,
        }
    }

    fn insert_child_maybe_after(
        &mut self,
        parent_index: usize,
        maybe_after_index: Option<usize>,
        child_value: T,
    ) -> usize {
        let child_index = self.nodes.len();
        let mut child_node = Node::new(child_value);

        child_node.parent = Some(parent_index);

        if let Some(after_node) = maybe_after_index.map(|i| &mut self.nodes[i]) {
            child_node.next_sibling = after_node.next_sibling;
            after_node.next_sibling = Some(child_index);
        } else {
            let parent_node = &mut self.nodes[parent_index];
            child_node.next_sibling = parent_node.first_child;
            parent_node.first_child = Some(child_index);
        }

        self.nodes.push(child_node);
        child_index
    }

    fn get_node(&self, index: usize) -> &Node<T> {
        &self.nodes[index]
    }

    fn get_node_mut(&mut self, index: usize) -> &mut Node<T> {
        &mut self.nodes[index]
    }
}

#[test]
fn test_insert_child_maybe_after() {
    let mut ntree = Tree::new(0);

    // insert first child / root
    let child1 = ntree.insert_child_maybe_after(ntree.root_index, None, 1);

    // insert second child after first
    let child2 = ntree.insert_child_maybe_after(ntree.root_index, Some(child1), 2);
    assert_eq!(ntree.get_node(child2).parent, Some(ntree.root_index));
    assert_eq!(ntree.get_node(child1).next_sibling, Some(child2));

    // insert third child after second
    let child3 = ntree.insert_child_maybe_after(ntree.root_index, Some(child2), 3);
    assert_eq!(ntree.get_node(child3).parent, Some(ntree.root_index));
    assert_eq!(ntree.get_node(child2).next_sibling, Some(child3));
    assert!(ntree.get_node(child3).next_sibling.is_none());
}

const DEFAULT_TEXTURE_WIDTH: u32 = 1024;
const DEFAULT_TEXTURE_HEIGHT: u32 = 1024;

#[derive(Debug)]
pub struct TexturePackerEntry {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,

    in_use: bool,
}

/// manages texture packing of textures as they are added.
#[derive(Debug)]
pub struct TexturePacker {
    w: u32,
    h: u32,
    gap: u32,

    tree: Tree<TexturePackerEntry>,
}

impl Default for TexturePacker {
    fn default() -> Self {
        Self::new(DEFAULT_TEXTURE_WIDTH, DEFAULT_TEXTURE_HEIGHT, 0)
    }
}

impl TexturePacker {
    pub fn new(texture_width: u32, texture_height: u32, gap: u32) -> Self {
        Self {
            w: texture_width,
            h: texture_height,
            gap,

            tree: Tree::new(TexturePackerEntry {
                x: 0,
                y: 0,
                w: texture_width,
                h: texture_height,

                in_use: false,
            }),
        }
    }

    fn is_leaf(&self, index: usize) -> bool {
        self.tree
            .get_node(index)
            .first_child
            .is_none_or(|h| self.tree.get_node(h).next_sibling.is_none())
    }

    fn is_left_child(&self, parent_index: usize, child_index: usize) -> bool {
        self.tree
            .get_node(parent_index)
            .first_child
            .is_some_and(|h| h == child_index)
    }

    fn is_right_child(&self, parent_index: usize, child_index: usize) -> bool {
        !self.is_left_child(parent_index, child_index)
    }

    fn insert_at(&mut self, width: u32, height: u32, index: usize) -> Option<usize> {
        if !self.is_leaf(index) {
            // try inserting under left child
            let left_child_index = self
                .tree
                .get_node(index)
                .first_child
                .expect("left child index");
            let new_index = self.insert_at(width, height, left_child_index);
            if new_index.is_some() {
                return new_index;
            }

            // no room, insert under right child
            let right_child_index = self
                .tree
                .get_node(left_child_index)
                .next_sibling
                .expect("right child index");
            return self.insert_at(width, height, right_child_index);
        }

        let entry = &self.tree.get_node(index).value;

        // there is already a glpyh here
        if entry.in_use {
            return None;
        }

        let cache_slot_width = entry.w;
        let cache_slot_height = entry.h;

        if width > cache_slot_width || height > cache_slot_height {
            // if this node's box is too small, return
            return None;
        }

        if width == cache_slot_width && height == cache_slot_height {
            // if we're just right, accept
            self.tree.get_node_mut(index).value.in_use = true;
            return Some(index);
        }

        // otherwise, gotta split this node and create some kids decide which way to split
        let dw = cache_slot_width - width;
        let dh = cache_slot_height - height;

        let (left_child, right_child) = if dw > dh {
            // split along x
            (
                TexturePackerEntry {
                    w: width,
                    h: cache_slot_height,
                    in_use: false,
                    ..*entry
                },
                TexturePackerEntry {
                    x: entry.x + width + self.gap,
                    w: dw - self.gap,
                    h: cache_slot_height,
                    in_use: false,
                    ..*entry
                },
            )
        } else {
            // split along y
            (
                TexturePackerEntry {
                    w: cache_slot_width,
                    h: height,
                    in_use: false,
                    ..*entry
                },
                TexturePackerEntry {
                    y: entry.y + height + self.gap,
                    w: cache_slot_width,
                    h: dh - self.gap,
                    in_use: false,
                    ..*entry
                },
            )
        };

        let left_child_index = self.tree.insert_child_maybe_after(index, None, left_child);
        assert!(self.is_left_child(index, left_child_index));

        let right_child_index =
            self.tree
                .insert_child_maybe_after(index, Some(left_child_index), right_child);
        assert!(self.is_right_child(index, right_child_index));

        assert!(
            self.tree.get_node(left_child_index).parent
                == self.tree.get_node(right_child_index).parent
        );
        assert!(self.tree.get_node(left_child_index).parent == Some(index));

        // insert into first child we created
        self.insert_at(width, height, left_child_index)
    }

    pub fn insert(&mut self, width: u32, height: u32) -> Option<usize> {
        self.insert_at(width, height, self.tree.root_index)
    }

    pub fn get(&self, index: usize) -> &TexturePackerEntry {
        &self.tree.get_node(index).value
    }

    pub fn get_texture_size(&self) -> (u32, u32) {
        (self.w, self.h)
    }
}

#[test]
fn test_insert_exact_fit() {
    let mut packer = TexturePacker::default();

    let maybe_index = packer.insert(DEFAULT_TEXTURE_WIDTH, DEFAULT_TEXTURE_HEIGHT);
    assert!(maybe_index.is_some());

    let index = maybe_index.unwrap();
    let entry = &packer.tree.get_node(index).value;
    assert!(entry.in_use);
    assert_eq!(entry.x, 0);
    assert_eq!(entry.y, 0);
    assert_eq!(entry.w, DEFAULT_TEXTURE_WIDTH);
    assert_eq!(entry.h, DEFAULT_TEXTURE_HEIGHT);
}

#[test]
fn test_insert_too_large() {
    let mut packer = TexturePacker::default();

    let maybe_index = packer.insert(DEFAULT_TEXTURE_WIDTH, DEFAULT_TEXTURE_HEIGHT + 1);
    assert!(maybe_index.is_none());
}

#[test]
fn test_horizontal_split() {
    let mut packer = TexturePacker::default();

    // create a rectangle that will cause a horizontal split (width difference > height
    // difference)
    let maybe_index1 = packer.insert(400, DEFAULT_TEXTURE_HEIGHT);
    assert!(maybe_index1.is_some());

    // the root should now have two children
    assert!(!packer.is_leaf(packer.tree.root_index));

    // try inserting another rectangle in the remaining space
    let maybe_index2 = packer.insert(400, DEFAULT_TEXTURE_HEIGHT);
    assert!(maybe_index2.is_some());

    assert!(maybe_index1 != maybe_index2);
}

#[test]
fn test_vertical_split() {
    let mut packer = TexturePacker::default();

    // create a rectangle that will cause a vertical split (height difference > width
    // difference)
    let maybe_index1 = packer.insert(DEFAULT_TEXTURE_WIDTH, 400);
    assert!(maybe_index1.is_some());

    // the root should now have two children
    assert!(!packer.is_leaf(packer.tree.root_index));

    // try inserting another rectangle in the remaining space
    let maybe_index2 = packer.insert(DEFAULT_TEXTURE_WIDTH, 400);
    assert!(maybe_index2.is_some());

    assert!(maybe_index1 != maybe_index2);
}
