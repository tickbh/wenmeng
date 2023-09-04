
use rbtree::RBTree;
use webparse::{http::http2::frame::{Frame, Priority, PriorityFrame}, Binary};


pub struct PriorityQueue {
    pub must_send_queue: Vec<Frame<Binary>>,
    pub send_queue: RBTree<PriorityFrame<Binary>, ()>,
}