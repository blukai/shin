async function shinMain(canvasId) {
    const canvas = document.getElementById(canvasId);
    let wasm;

    function decodeCStr(ptr) {
        let bytes = new Uint8Array(wasm.exports.memory.buffer, ptr);
        let str = "";
        for (i = 0; bytes[i]; i += 1) {
            var byte = bytes[i];
            str += String.fromCharCode(byte);
        }
        return str;
    }

    const imports = {
        env: {
            panic: (ptr) => {
                // TODO: do something more prominent on panic.
                throw new Error(decodeCStr(ptr));
            },
            console_log: (ptr) => {
                console.log(decodeCStr(ptr));
            },
            request_animation_frame_loop: (f, ctx) => {
                function tick() {
                    if (wasm.exports.__indirect_function_table.get?.(f)(ctx)) {
                        requestAnimationFrame(tick);
                    }
                }
                requestAnimationFrame(tick);
            },
            resize_canvas: (width, height) => {
                canvas.width = width;
                canvas.height = height;
            },
        },
    };
    const wasmStream = fetch("/target/wasm32-unknown-unknown/debug/examples/second.wasm");
    const wasmMod = await WebAssembly.compileStreaming(wasmStream);
    wasm = await WebAssembly.instantiate(wasmMod, imports);

    wasm.exports.main();

    window.__shin = {
        wasmMod,
        wasm,
        canvas,
    };
}
