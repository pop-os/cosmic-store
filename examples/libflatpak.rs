use libflatpak::{gio::Cancellable, prelude::*, Installation, Transaction};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let inst = Installation::new_user(Cancellable::NONE)?;
    let tx = Transaction::for_installation(&inst, Cancellable::NONE)?;
    tx.connect_new_operation(|_, op, progress| {
        println!("Operation: {:?} {:?}", op.operation_type(), op.get_ref());
        progress.connect_changed(|progress| {
            println!("Progress: {}", progress.progress());
        });
    });
    tx.add_install("flathub", "app/com.system76.Popsicle/x86_64/stable", &[])?;
    tx.run(Cancellable::NONE)?;

    Ok(())
}
