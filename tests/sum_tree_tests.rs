use zed_text_editor::tree::{Count, Item, SumTree};

#[derive(Clone, Debug, PartialEq)]
struct TestItem(usize);

impl Item for TestItem {
    type Summary = Count;

    fn summary(&self) -> Count {
        Count { value: self.0 }
    }
}

#[test]
fn test_empty_tree() {
    let tree: SumTree<TestItem> = SumTree::new();
    assert!(tree.is_empty());
    assert_eq!(tree.summary().value, 0);
}

#[test]
fn test_push_single_item() {
    let mut tree = SumTree::new();
    tree.push(TestItem(42));

    assert!(!tree.is_empty());
    assert_eq!(tree.summary().value, 42);
}

#[test]
fn test_push_multiple_items() {
    let mut tree = SumTree::new();
    tree.push(TestItem(10));
    tree.push(TestItem(20));
    tree.push(TestItem(30));

    assert_eq!(tree.summary().value, 60);
}
