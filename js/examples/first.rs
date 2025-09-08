use std::{cell::Cell, mem::ManuallyDrop, rc::Rc};

fn panic_hook(info: &std::panic::PanicHookInfo) {
    let msg = info.to_string();
    unsafe { js::throw_str(msg.as_ptr(), msg.len()) };
}

fn main() {
    std::panic::set_hook(Box::new(panic_hook));

    js::global()
        .get("console")
        .get("log")
        .call(&[js::Value::from_str("hello, sailor!")])
        .unwrap();

    let count = Rc::new(Cell::new(0_usize));

    // NOTE: (on ManuallyDrop) this function is shorter then the time closures need to live to be
    // executed. here we register then with the js and js decides when they'll be executed; but for
    // them to be executed they need to live long enough. ManuallyDrop is not what you should use,
    // but for this example i don't care enought i guess.
    //
    // if you can think of something better - do it.

    let animation_frame_callback = ManuallyDrop::new(js::Closure::new({
        let count = Rc::clone(&count);
        move || {
            count.update(|prev| prev + 1);
            js::global()
                .get("console")
                .get("log")
                .call(&[js::Value::from_str(&format!(
                    "new count: {:?}",
                    count.get()
                ))])
                .unwrap();
        }
    }));
    js::global()
        .get("requestAnimationFrame")
        .call(&[js::Value::from_closure(&animation_frame_callback)])
        .unwrap();

    let timeout_callback = ManuallyDrop::new(js::Closure::new({
        let count = Rc::clone(&count);
        move || {
            js::global()
                .get("console")
                .get("log")
                .call(&[js::Value::from_str(&format!(
                    "{count:?}; strong_count: {strong_count}",
                    strong_count = Rc::strong_count(&count)
                ))])
                .unwrap();
        }
    }));
    js::global()
        .get("setTimeout")
        .call(&[
            js::Value::from_closure(&timeout_callback),
            js::Value::from_f64(1000.0),
        ])
        .unwrap();

    js::global()
        .get("document")
        .get("body")
        .get("style")
        .set("background", &js::Value::from_str("black"));
}
