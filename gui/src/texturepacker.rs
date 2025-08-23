#[derive(Debug)]
struct Node<T> {
    value: T,
    parent: Option<usize>,
    first_child: Option<usize>,
    // NOTE: having prev_sibling enables cheap unlink/remove.
    prev_sibling: Option<usize>,
    next_sibling: Option<usize>,
}

impl<T> Node<T> {
    fn new(value: T) -> Self {
        Self {
            value: value,
            parent: None,
            first_child: None,
            prev_sibling: None,
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

    fn try_get_node(&self, index: usize) -> Option<&Node<T>> {
        self.nodes.get(index).and_then(|node| node.as_ref())
    }

    fn get_node(&self, index: usize) -> &Node<T> {
        self.try_get_node(index).expect("invalid index")
    }

    fn try_get_node_mut(&mut self, index: usize) -> Option<&mut Node<T>> {
        self.nodes.get_mut(index).and_then(|node| node.as_mut())
    }

    fn get_node_mut(&mut self, index: usize) -> &mut Node<T> {
        self.try_get_node_mut(index).expect("invalid index")
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
        child_node.prev_sibling = maybe_after_index;

        if let Some(after_node) = maybe_after_index.map(|i| self.get_node_mut(i)) {
            child_node.next_sibling = after_node.next_sibling;
            after_node.next_sibling = Some(child_index);
        } else {
            let parent_node = self.get_node_mut(parent_index);
            child_node.next_sibling = parent_node.first_child;
            parent_node.first_child = Some(child_index);
        }

        if let Some(next_sibling_node) = child_node.next_sibling.map(|i| self.get_node_mut(i)) {
            next_sibling_node.prev_sibling = Some(child_index);
        }

        self.nodes[child_index] = Some(child_node);
        child_index
    }

    fn remove_node(&mut self, index: usize) {
        let node = self.nodes[index].take().expect("invalid index");
        self.free_indices.push(index);

        // if we're the first guy, reset the head otherwise, make our previous node's next pointer
        // = our next
        if let Some(prev_sibling_node) = node.prev_sibling.map(|i| self.get_node_mut(i)) {
            prev_sibling_node.next_sibling = node.next_sibling;
        } else {
            if let Some(parent_node) = node.parent.map(|i| self.get_node_mut(i)) {
                parent_node.first_child = node.next_sibling;
            } else if self.root_index == index {
                // TODO: consider not panicking when removing root?
                self.root_index = node.next_sibling.expect("next sibling");
            }
        }

        // if we're the last guy, reset the tail otherwise, make our next node's prev pointer = our
        // prev
        if let Some(next_sibling_node) = node.next_sibling.map(|i| self.get_node_mut(i)) {
            next_sibling_node.prev_sibling = node.prev_sibling;
        }
    }
}

#[test]
fn test_insert_child_maybe_after() {
    let mut tree = Tree::new(0);

    // insert first child / root
    let child1 = tree.insert_child_maybe_after(tree.root_index, None, 1);
    assert_eq!(tree.get_node(child1).parent, Some(tree.root_index));
    assert_eq!(tree.get_node(tree.root_index).first_child, Some(child1));

    // insert second child after first
    let child2 = tree.insert_child_maybe_after(tree.root_index, Some(child1), 2);
    assert_eq!(tree.get_node(child2).parent, Some(tree.root_index));
    assert_eq!(tree.get_node(child2).prev_sibling, Some(child1));
    assert_eq!(tree.get_node(child1).next_sibling, Some(child2));

    // insert third child after second
    let child3 = tree.insert_child_maybe_after(tree.root_index, Some(child2), 3);
    assert_eq!(tree.get_node(child3).parent, Some(tree.root_index));
    assert_eq!(tree.get_node(child3).prev_sibling, Some(child2));
    assert_eq!(tree.get_node(child2).next_sibling, Some(child3));
    assert_eq!(tree.get_node(child3).next_sibling, None);
}

#[test]
fn test_remove_node() {
    let mut tree = Tree::new(0);

    let child1 = tree.insert_child_maybe_after(tree.root_index, None, 1);
    let child2 = tree.insert_child_maybe_after(tree.root_index, Some(child1), 2);
    let child3 = tree.insert_child_maybe_after(tree.root_index, Some(child2), 3);

    tree.remove_node(child2);
    assert!(tree.try_get_node(child2).is_none());
    // check that child1 now points to child3
    assert_eq!(tree.get_node(child1).next_sibling, Some(child3));
    assert_eq!(tree.get_node(child3).prev_sibling, Some(child1));

    tree.remove_node(child1);
    assert!(tree.try_get_node(child1).is_none());
    // check that root now points to child3 as first child
    assert_eq!(tree.get_node(tree.root_index).first_child, Some(child3));
    assert_eq!(tree.get_node(child3).prev_sibling, None);

    tree.remove_node(child3);
    assert!(tree.try_get_node(child3).is_none());
    // check that root has no children
    assert_eq!(tree.get_node(tree.root_index).first_child, None);
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

    pub fn remove(&mut self, index: usize) {
        self.tree.get_node_mut(index).value.in_use = false;

        if !self.is_leaf(index) {
            return;
        }

        // if its a leaf, see if its peer is empty, if it is the split can go away.
        let parent_index = self.tree.get_node(index).parent.expect("parent index");
        match () {
            _ if self.is_left_child(parent_index, index) => {
                if let Some(peer_index) = self.tree.get_node(index).next_sibling {
                    assert!(self.is_right_child(index, peer_index));
                    if self.is_leaf(peer_index) && !self.tree.get_node(peer_index).value.in_use {
                        // both children are leaves and neither is in use, remove the split here.
                        self.tree.remove_node(index);
                        self.tree.remove_node(peer_index);
                    }
                }
            }
            _ if self.is_right_child(parent_index, index) => {
                if let Some(peer_index) = self.tree.get_node(parent_index).first_child {
                    assert!(self.is_left_child(parent_index, peer_index));
                    assert_eq!(Some(index), self.tree.get_node(peer_index).next_sibling);
                    if self.is_leaf(peer_index) && !self.tree.get_node(peer_index).value.in_use {
                        // both children are leaves and neither is in use, remove the split here.
                        self.tree.remove_node(index);
                        self.tree.remove_node(peer_index);
                    }
                }
            }
            _ => unreachable!(),
        }
        // maybe parent (that is not a root) is now empty.
        if self.is_leaf(parent_index) && parent_index != self.tree.root_index {
            self.remove(parent_index);
        }
    }

    pub fn try_get(&self, index: usize) -> Option<&TexturePackerEntry> {
        self.tree.try_get_node(index).map(|node| &node.value)
    }

    pub fn get(&self, index: usize) -> &TexturePackerEntry {
        self.try_get(index).expect("invalud index")
    }

    pub fn texture_size(&self) -> (u32, u32) {
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

#[test]
fn test_remove_leaf_node() {
    let mut packer = TexturePacker::default();

    let index = packer.insert(400, 400).expect("failed to insert");
    assert!(packer.tree.get_node(index).value.in_use);
    assert!(packer.is_leaf(index));

    packer.remove(index);
    assert!(packer.try_get(index).is_none());
    assert!(packer.is_leaf(packer.tree.root_index));
}
