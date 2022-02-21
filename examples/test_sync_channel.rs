use std::{sync::mpsc::channel, thread};

fn main() {
    let (tx, rx) = channel::<i32>();

    tx.send(1).unwrap();
    tx.send(2).unwrap();
    tx.send(3).unwrap();
    drop(tx);

    thread::spawn(move || loop {
        let x = rx.try_recv();
        println!("{:?}", x);
        if x.is_err() {
            break;
        }
    })
    .join()
    .unwrap();
}
