use crate::collider;
use raylib::core::math::Vector2;

type EntityData<'a> = (
    &'a collider::Collider,
    raylib::prelude::Vector2,
    collider::AABB,
    u32,
);

/// Splits the list of entities into two lists by the mean position in the specified axis.
fn split_at_mid(mut v: Vec<EntityData>, x_axis: bool) -> (Vec<EntityData>, Vec<EntityData>) {
    let result: (&mut [EntityData], &mut EntityData, &mut [EntityData]);
    let half_size = (v.len() / 2usize) - 1;

    if x_axis {
        result = v.select_nth_unstable_by(half_size, |vec1, vec2| {
            vec1.1.x.partial_cmp(&vec2.1.x).unwrap()
        });
    } else {
        result = v.select_nth_unstable_by(half_size, |vec1, vec2| {
            vec1.1.y.partial_cmp(&vec2.1.y).unwrap()
        });
    }
    let mut start = result.0.to_vec();
    start.push(result.1.clone());
    let end = result.2.to_vec();
    assert_eq!(start.len() + end.len(), v.len());
    (start, end)
}

/// A single node in the bounding volume hierarchy.
/// The node can eighter contain children, or collision data.
// for each node half of the bounding box data is shared with the parent node, so some effeciency improvements can be made here.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Branch(collider::AABB, [Box<Node>; 2]),
    Fruit(collider::AABB, u32, [bool; collider::LAYERS]),
}

impl Node {
    /// Creates a node from some entities
    fn new(mut data: Vec<EntityData>) -> Node {
        // if there is only a single node, return a fruit node
        if data.len() <= 1 {
            let owned = data.remove(0);
            Node::Fruit(owned.2, owned.3, owned.0.collision_layers)
        } else {
            // otherwise, split the list into two halves based on the axis with the widest position spread and create a branch node
            let mut total_bb = data[0].2.clone();
            for e in &data {
                total_bb = total_bb.with_point(&e.1);
            }
            let (first_half, second_half) = split_at_mid(
                data,
                (total_bb.rx - total_bb.lx) > (total_bb.ry - total_bb.ly),
            );
            let node1 = Node::new(first_half);
            let node2 = Node::new(second_half);
            let bb1 = match node1 {
                Node::Branch(ref bb, _) => bb,
                Node::Fruit(ref bb, _, _) => bb,
            };
            let bb2 = match node2 {
                Node::Branch(ref bb, _) => bb,
                Node::Fruit(ref bb, _, _) => bb,
            };
            let total_bb = bb1.get_union(bb2);
            Node::Branch(total_bb, [Box::new(node1), Box::new(node2)])
        }
    }

    /// Shrink the bounding box to the smallest bounding box that contains all the entities in the node.
    fn shrink(&mut self) -> &collider::AABB {
        match self {
            Node::Branch(bb, children) => {
                let mut children_iter = children.iter_mut();
                if let Some(first) = children_iter.next() {
                    *bb = first.shrink().clone();
                    for c in children_iter {
                        *bb = bb.get_union(c.shrink());
                    }
                };
                bb
            }
            Node::Fruit(bb, _, _) => bb,
        }
    }

    /// Get the ids of all of the entities under this node.
    fn get_children_id(&self) -> Vec<u32> {
        let mut sum_vec = Vec::new();
        match self {
            Node::Branch(_, children) => {
                for c in children {
                    sum_vec.append(&mut c.get_children_id());
                }
            }
            Node::Fruit(_, other_data, _) => {
                sum_vec.push(*other_data);
            }
        }
        sum_vec
    }

    /// Get the entities under this node.
    fn get_children(&self) -> Vec<&Node> {
        let mut sum_vec = Vec::new();
        if let Node::Branch(_, children) = self {
            for c in children {
                sum_vec.append(&mut c.get_children());
            }
        }
        sum_vec.push(self);
        sum_vec
    }

    /// Traverse the tree based on a custom collision callback. Callback and current state allows state to be held as the tree is traversed.
    fn traverse<'a, T: Clone, K>(
        &'a self,
        p: &K,
        layers: &[bool; collider::LAYERS],
        collision_callback: fn(&collider::AABB, &K) -> bool,
        callback: &mut impl FnMut(&'a Node, T) -> T,
        current_state: T,
    ) {
        match self {
            Node::Branch(bb, children) => {
                if collision_callback(bb, &p) {
                    let next_state = callback(self, current_state);
                    for child in children {
                        child.traverse(p, layers, collision_callback, callback, next_state.clone());
                    }
                }
            }
            Node::Fruit(bb, _, l) => {
                let mut contains_layer = false;
                for (layer1, layer2) in l.iter().zip(layers) {
                    if *layer1 && *layer2 {
                        contains_layer = true;
                        break;
                    }
                }
                if contains_layer && collision_callback(bb, &p) {
                    callback(self, current_state);
                }
            }
        }
    }

    /// Traverse any nodes that collide with a point.
    fn traverse_point<'a, T: Clone>(
        &'a self,
        p: &Vector2,
        layers: &[bool; collider::LAYERS],
        callback: &mut impl FnMut(&'a Node, T) -> T,
        current_state: T,
    ) {
        fn collide_point(bb: &collider::AABB, p: &Vector2) -> bool {
            bb.lx < p.x && bb.rx > p.x && bb.ly < p.y && bb.ry > p.y
        }
        self.traverse(p, layers, collide_point, callback, current_state);
    }

    /// Traverse any nodes that collide with a bounding box.
    fn traverse_rect<'a, T: Clone>(
        &'a self,
        r: &collider::AABB,
        layers: &[bool; collider::LAYERS],
        callback: &mut impl FnMut(&'a Node, T) -> T,
        current_state: T,
    ) {
        fn collide_rect(bb: &collider::AABB, bb2: &collider::AABB) -> bool {
            bb.is_colliding(bb2)
        }
        self.traverse(r, layers, collide_rect, callback, current_state);
    }

    /// Find any entities that collide with a point.
    fn query_point(&self, p: &Vector2, layers: &[bool; collider::LAYERS]) -> Option<Vec<u32>> {
        let mut result: Option<Vec<u32>> = None;
        self.traverse_point(
            p,
            layers,
            &mut |node, _| match node {
                Node::Branch(_, _) => (),
                Node::Fruit(_, other_data, _) => {
                    if let Some(ref mut result_vec) = result {
                        result_vec.push(*other_data);
                    } else {
                        result = Some(vec![*other_data]);
                    }
                }
            },
            (),
        );
        result
    }

    /// Find any entities that collide with a point, and what nodes contain those enties. Useful for debugging purposes.
    fn debug_query_point<'a>(
        &'a self,
        p: &Vector2,
        layers: &[bool; collider::LAYERS],
    ) -> (Option<Vec<u32>>, Vec<(&Node, i32)>) {
        let mut result: (Option<Vec<u32>>, Vec<(&'a Node, i32)>) = (None, Vec::new());
        self.traverse_point(
            p,
            layers,
            &mut |node, depth| {
                result.1.push((&node, depth));
                match node {
                    Node::Branch(_, _) => {}
                    Node::Fruit(_, other_data, _) => {
                        if let Some(ref mut result_vec) = result.0 {
                            result_vec.push(*other_data);
                        } else {
                            result.0 = Some(vec![*other_data]);
                        }
                    }
                }
                depth + 1
            },
            0i32,
        );
        result
    }

    /// Find any entities that collide with a bounding box.
    fn query_rect(
        &self,
        r: &collider::AABB,
        layers: &[bool; collider::LAYERS],
    ) -> Option<Vec<u32>> {
        let mut result: Option<Vec<u32>> = None;
        self.traverse_rect(
            r,
            layers,
            &mut |node, _| match node {
                Node::Branch(_, _) => (),
                Node::Fruit(_, other_data, _) => {
                    if let Some(ref mut result_vec) = result {
                        result_vec.push(*other_data);
                    } else {
                        result = Some(vec![*other_data]);
                    }
                }
            },
            (),
        );
        result
    }

    /// Find any entities that collide with a bounding box, and what nodes contain those enties. Useful for debugging purposes.
    fn debug_query_rect<'a>(
        &'a self,
        r: &collider::AABB,
        layers: &[bool; collider::LAYERS],
    ) -> (Option<Vec<u32>>, Vec<(&Node, i32)>) {
        let mut result: (Option<Vec<u32>>, Vec<(&'a Node, i32)>) = (None, Vec::new());
        self.traverse_rect(
            r,
            layers,
            &mut |node, depth| {
                result.1.push((&node, depth));
                match node {
                    Node::Branch(_, _) => {}
                    Node::Fruit(_, other_data, _) => {
                        if let Some(ref mut result_vec) = result.0 {
                            result_vec.push(*other_data);
                        } else {
                            result.0 = Some(vec![*other_data]);
                        }
                    }
                }
                depth.clone() + 1
            },
            0i32,
        );
        result
    }

    /// Update a bounding box within the tree.
    /// Note: This does will only grow the bounding boxes of the parent nodes, so it it important to call shrink regularly to maintain optimal bounding boxes for branch nodes.
    fn update(&mut self, old: (&collider::AABB, u32), new: (&collider::AABB, u32)) -> bool {
        match self {
            Node::Branch(bb, children) => {
                if bb.contains(old.0) {
                    for c in children {
                        if c.update(old, new) {
                            return true;
                        }
                    }
                    *bb = bb.get_union(new.0);
                }
            }
            Node::Fruit(bb, id, _) => {
                if *id == old.1 {
                    *bb = new.0.clone();
                    return true;
                }
            }
        }
        false
    }

    /// Remove a entity from the tree.
    fn delete(&mut self, old: u32) -> (bool, bool) {
        match self {
            Node::Branch(_, children) => {
                let result = children[0].delete(old);
                if result.1 {
                    if result.0 {
                        *self = *children[1].clone();
                    }
                    return (false, true);
                }
                let result = children[1].delete(old);
                if result.1 {
                    if result.0 {
                        *self = *children[0].clone();
                    }
                    return (false, true);
                }
            }
            Node::Fruit(_, id, _) => {
                if *id == old {
                    return (true, true);
                }
            }
        }
        (false, false)
    }

    /// Create a new entity in the tree.
    fn insert(&mut self, new: &(&collider::Collider, Vector2, collider::AABB, u32)) {
        let new_fruit_bb = new.0.get_bounding_box(&new.1);
        match self {
            Node::Branch(bb, children) => {
                *bb = bb.get_union(&new_fruit_bb);
                let mut best_dist = match &*children[0] {
                    Node::Branch(bb2, _) => bb2.get_dist(&new.2),
                    Node::Fruit(bb2, _, _) => bb2.get_dist(&new.2),
                };
                let (ref mut first, rest) = children.split_at_mut(1);
                let mut best = &mut *first[0];
                for child in rest {
                    let new_dist = match &**child {
                        Node::Branch(bb2, _) => bb2.get_dist(&new.2),
                        Node::Fruit(bb2, _, _) => bb2.get_dist(&new.2),
                    };
                    if new_dist < best_dist {
                        best_dist = new_dist;
                        best = &mut **child;
                    }
                }
                best.insert(new);
            }
            Node::Fruit(bb, _, _) => {
                let new_branch_bb = new_fruit_bb.get_union(bb);
                *self = Node::Branch(
                    new_branch_bb,
                    [
                        Box::new(Node::Fruit(new_fruit_bb, new.3, new.0.collision_layers)),
                        Box::new(self.clone()),
                    ],
                );
            }
        }
    }
}

/// This handles broad phase optimization of collisions.
/// It is a bounding volume hierarchy constructed top-down with 2 subdivisions.
pub struct BVHTree {
    root_node: Node,
}

impl BVHTree {
    /// Create a new BVHTree from the enties.
    pub fn new(data: Vec<EntityData>) -> BVHTree {
        BVHTree {
            root_node: Node::new(data),
        }
    }

    /// Get all the entity ids in the tree.
    pub fn get_children_id(&self) -> Vec<u32> {
        self.root_node.get_children_id()
    }

    /// Get all the entities in the tree.
    pub fn get_children(&self) -> Vec<&Node> {
        self.root_node.get_children()
    }

    /// Get all the entity ids in the tree that collide with a point.
    pub fn query_point(&self, p: &Vector2, layers: &[bool; collider::LAYERS]) -> Vec<u32> {
        self.root_node.query_point(p, layers).unwrap_or_default()
    }

    /// Get all the entity ids in the tree that collide with a bounding box.
    pub fn query_rect(&self, r: &collider::AABB, layers: &[bool; collider::LAYERS]) -> Vec<u32> {
        self.root_node.query_rect(r, layers).unwrap_or_default()
    }

    /// Get all the entity ids along with the nodes that contian them in the tree that collide with a bounding box.
    pub fn debug_query_rect(
        &self,
        r: &collider::AABB,
        layers: &[bool; collider::LAYERS],
    ) -> (Option<Vec<u32>>, Vec<(&Node, i32)>) {
        self.root_node.debug_query_rect(r, layers)
    }

    /// Get all the entity ids along with the nodes that contian them in the tree that collide with a point.
    pub fn debug_query_point(
        &self,
        p: &Vector2,
        layers: &[bool; collider::LAYERS],
    ) -> (Option<Vec<u32>>, Vec<(&Node, i32)>) {
        self.root_node.debug_query_point(p, layers)
    }

    /// Update a bounding box within the tree.
    pub fn update(&mut self, old: (&collider::AABB, u32), new: (&collider::AABB, u32)) {
        self.root_node.update(old, new);
    }

    /// Create a new entity in the tree.
    pub fn insert(&mut self, new: &(&collider::Collider, Vector2, collider::AABB, u32)) {
        self.root_node.insert(new);
    }

    /// Remove a entity from the tree.
    pub fn delete(&mut self, old: u32) {
        self.root_node.delete(old);
    }

    /// Shrink all of the bounding boxes in the tree to the minimal size.
    pub fn shrink(&mut self) {
        self.root_node.shrink();
    }
}
