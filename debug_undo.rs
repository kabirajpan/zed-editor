use zed_text_editor::Editor;

fn main() {
    let mut editor = Editor::new();

    println!("Initial: text='{}', can_undo={}", editor.text(), editor.can_undo());

    editor.insert("h");
    println!("After 'h': text='{}', can_undo={}", editor.text(), editor.can_undo());

    editor.insert("e");
    editor.insert("l");
    editor.insert("l");
    editor.insert("o");
    println!("After 'hello': text='{}', can_undo={}", editor.text(), editor.can_undo());

    editor.insert(" ");
    println!("After ' ': text='{}', can_undo={}", editor.text(), editor.can_undo());

    println!("Before undo: can_undo={}", editor.can_undo());
    editor.undo();
    println!("After undo: text='{}', can_undo={}", editor.text(), editor.can_undo());
}
