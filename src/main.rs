use zed_text_editor::Editor;

fn main() {
    println!("🚀 Zed-Style Text Editor - With Undo/Redo!\n");

    let mut editor = Editor::new();

    // Type some text
    println!("📝 Typing...");
    editor.insert("Hello");
    println!("   Text: {:?}", editor.text());

    editor.insert(" ");
    editor.insert("World");
    println!("   Text: {:?}", editor.text());

    // Undo
    println!("\n⏪ Undo...");
    editor.undo();
    println!("   Text: {:?}", editor.text());

    editor.undo();
    println!("   Text: {:?}", editor.text());

    // Redo
    println!("\n⏩ Redo...");
    editor.redo();
    println!("   Text: {:?}", editor.text());

    editor.redo();
    println!("   Text: {:?}", editor.text());

    println!("\n🎉 Full editor with undo/redo working!");
}
