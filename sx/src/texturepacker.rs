// NOTE: this texture packer doesn't let you remove or replace textures. it packs things as tightly
// as possible which results in high degree of fragmentation.
//
// during removal you'll need to merge splits and possibly re-arrange them, this will require
// potentially full re-build and re-upload.
// it is easier to create a new "pack" instead of micro-managing.
//
// if you want texture packer with support for removal you must look elsewhere; this one probably
// cannot be adapted to those needs.

// NOTE: Node is marked as non_exhaustive because i want to be able to expose it and its fields for
// debuging purposes, but i do not want it to be constructable from outside.
#[non_exhaustive]
#[derive(Debug)]
pub struct Node<T> {
    pub value: T,
    pub parent: Option<usize>,
    pub first_child: Option<usize>,
    pub next_sibling: Option<usize>,
}

impl<T> Node<T> {
    fn new(value: T) -> Self {
        Self {
            value: value,
            parent: None,
            first_child: None,
            next_sibling: None,
        }
    }
}

/// a stupid n-way tree
///
/// NOTE: Tree is based on Valve's CUtlNTree.
#[derive(Debug)]
struct Tree<T> {
    nodes: Vec<Option<Node<T>>>,
    root_index: usize,
    free_indices: Vec<usize>,
}

impl<T> Tree<T> {
    fn new(value: T) -> Self {
        Self {
            nodes: vec![Some(Node::new(value))],
            root_index: 0,
            free_indices: vec![],
        }
    }

    fn try_get(&self, index: usize) -> Option<&Node<T>> {
        self.nodes.get(index).and_then(|node| node.as_ref())
    }

    fn get(&self, index: usize) -> &Node<T> {
        self.try_get(index).expect("invalid index")
    }

    fn try_get_mut(&mut self, index: usize) -> Option<&mut Node<T>> {
        self.nodes.get_mut(index).and_then(|node| node.as_mut())
    }

    fn get_mut(&mut self, index: usize) -> &mut Node<T> {
        self.try_get_mut(index).expect("invalid index")
    }

    fn iter(&self) -> impl Iterator<Item = (usize, &Node<T>)> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.as_ref().map(|node| (i, node)))
    }

    fn insert_child_maybe_after(
        &mut self,
        parent_index: usize,
        maybe_after_index: Option<usize>,
        child_value: T,
    ) -> usize {
        let child_index = self.free_indices.pop().unwrap_or_else(|| {
            let ret = self.nodes.len();
            self.nodes.push(None);
            ret
        });
        let mut child_node = Node::new(child_value);

        child_node.parent = Some(parent_index);

        if let Some(after_node) = maybe_after_index.map(|i| self.get_mut(i)) {
            child_node.next_sibling = after_node.next_sibling;
            after_node.next_sibling = Some(child_index);
        } else {
            let parent_node = self.get_mut(parent_index);
            child_node.next_sibling = parent_node.first_child;
            parent_node.first_child = Some(child_index);
        }

        self.nodes[child_index] = Some(child_node);
        child_index
    }
}

#[test]
fn test_insert_child_maybe_after() {
    let mut tree = Tree::new(0);

    // insert first child / root
    let child1 = tree.insert_child_maybe_after(tree.root_index, None, 1);
    assert_eq!(tree.get(child1).parent, Some(tree.root_index));
    assert_eq!(tree.get(tree.root_index).first_child, Some(child1));

    // insert second child after first
    let child2 = tree.insert_child_maybe_after(tree.root_index, Some(child1), 2);
    assert_eq!(tree.get(child2).parent, Some(tree.root_index));
    assert_eq!(tree.get(child1).next_sibling, Some(child2));

    // insert third child after second
    let child3 = tree.insert_child_maybe_after(tree.root_index, Some(child2), 3);
    assert_eq!(tree.get(child3).parent, Some(tree.root_index));
    assert_eq!(tree.get(child2).next_sibling, Some(child3));
    assert_eq!(tree.get(child3).next_sibling, None);
}

const DEFAULT_TEXTURE_WIDTH: u32 = 1024;
const DEFAULT_TEXTURE_HEIGHT: u32 = 1024;

#[derive(Debug)]
pub struct TexturePackerEntry {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,

    pub in_use: bool,
}

/// manages texture packing of textures as they are added.
///
/// NOTE: TexturePacker is based on Valve's CTexturePacker.
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
            .get(index)
            .first_child
            .is_none_or(|h| self.tree.get(h).next_sibling.is_none())
    }

    fn is_left_child(&self, parent_index: usize, child_index: usize) -> bool {
        self.tree
            .get(parent_index)
            .first_child
            .is_some_and(|h| h == child_index)
    }

    fn is_right_child(&self, parent_index: usize, child_index: usize) -> bool {
        !self.is_left_child(parent_index, child_index)
    }

    fn insert_at(&mut self, width: u32, height: u32, index: usize) -> Option<usize> {
        if !self.is_leaf(index) {
            // try inserting under left child
            let left_child_index = self.tree.get(index).first_child.expect("left child index");
            let new_index = self.insert_at(width, height, left_child_index);
            if new_index.is_some() {
                return new_index;
            }

            // no room, insert under right child
            let right_child_index = self
                .tree
                .get(left_child_index)
                .next_sibling
                .expect("right child index");
            return self.insert_at(width, height, right_child_index);
        }

        let entry = &self.tree.get(index).value;

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
            self.tree.get_mut(index).value.in_use = true;
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
                    // TODO: what if dw > gap? this will panic.
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
                    // TODO: what if dh > gap? this will panic.
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

        assert!(self.tree.get(left_child_index).parent == self.tree.get(right_child_index).parent);
        assert!(self.tree.get(left_child_index).parent == Some(index));

        // insert into first child we created
        self.insert_at(width, height, left_child_index)
    }

    pub fn insert(&mut self, width: u32, height: u32) -> Option<usize> {
        self.insert_at(width, height, self.tree.root_index)
    }

    pub fn try_get_entry(&self, index: usize) -> Option<&TexturePackerEntry> {
        self.tree.try_get(index).map(|node| &node.value)
    }

    pub fn get_entry(&self, index: usize) -> &TexturePackerEntry {
        self.try_get_entry(index).expect("invalid index")
    }

    pub fn texture_size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    pub fn is_empty(&self) -> bool {
        // NOTE: tree can not exist without a root.
        assert!(self.tree.nodes.len() > 0);
        self.tree.nodes.len() == 1
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = (usize, &Node<TexturePackerEntry>)> {
        self.tree.iter()
    }
}

#[test]
fn test_insert_exact_fit() {
    let mut packer = TexturePacker::default();

    let maybe_index = packer.insert(DEFAULT_TEXTURE_WIDTH, DEFAULT_TEXTURE_HEIGHT);
    assert!(maybe_index.is_some());

    let index = maybe_index.unwrap();
    let entry = &packer.tree.get(index).value;
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
fn test_insert_horizontal_split() {
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
fn test_insert_vertical_split() {
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
