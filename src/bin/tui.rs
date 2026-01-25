use zed_text_editor::ui::{App, init, restore, render};
use std::io;

fn main() -> io::Result<()> {
    // Initialize terminal
    let mut terminal = init()?;
    
    // Create app
    let mut app = App::new();
    
    // Main loop
    let result = run_app(&mut terminal, &mut app);
    
    // Restore terminal
    restore()?;
    
    result
}

fn run_app(terminal: &mut zed_text_editor::ui::Tui, app: &mut App) -> io::Result<()> {
    loop {
        // Draw UI
        terminal.draw(|frame| render(app, frame))?;
        
        // Handle input
        app.handle_input()?;
        
        // Check if should quit
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
