async function shinInit(path) {
    const textDecoder = new TextDecoder();

    let wasm;

    function assert(truth, msg) {
        if (!truth) {
            throw new Error(`assertion failed${msg ? `: ${msg}` : ""}`);
        }
    }

    let wasmUint8Array = null;
    function getWasmUint8Array() {
        if (!wasmUint8Array || !wasmUint8Array.byteLength) {
            wasmUint8Array = new Uint8Array(wasm.exports.memory.buffer);
        }
        return wasmUint8Array;
    }

    let wasmDataView = null;
    function getWasmDataView() {
        if (!wasmDataView || wasmDataView.buffer !== wasm.exports.memory.buffer) {
            wasmDataView = new DataView(wasm.exports.memory.buffer);
        }
        return wasmDataView;
    }

    function wasmReadCStr(ptr) {
        const len = getWasmUint8Array().subarray(ptr).findIndex((b) => !b);
        return textDecoder.decode(getWasmUint8Array().subarray(ptr, ptr + len));
    }

    function wasmWriteI32(ptr, value) {
        getWasmDataView().setInt32(ptr, value, true);
    }

    // NOTE: it does not seem like rust supports exteralref thing. 
    //
    // NOTE: i want to treat 0 as null/undefined, thus i'm faking 1-based
    // indices.
    const externRefGlue = {
        table: new WebAssembly.Table({ element: "externref", initial: 0 }),
        freeIndices: [],
        grow: (delta) => {
            const oldLen = externRefGlue.table.length;
            externRefGlue.table.grow(delta);
            for (let i = 0; i < delta; i += 1) {
                externRefGlue.freeIndices.push(oldLen + i);
            }
        },
        alloc: () => {
            const freeIdx = externRefGlue.freeIndices.pop();
            if (freeIdx !== undefined) {
                return freeIdx;
            }
            assert(externRefGlue.freeIndices.length === 0);
            externRefGlue.grow(Math.max(externRefGlue.freeIndices.length * 2, 1));
            return externRefGlue.freeIndices.pop();
        },
        insert: (value) => {
            const idx = externRefGlue.alloc();
            externRefGlue.table.set(idx, value);
            return idx + 1;
        },
        get: (idx) => externRefGlue.table.get(idx - 1),
    };

    const imports = {
        env: {
            panic: (ptr) => {
                // TODO: do something more prominent on panic.
                throw new Error(wasmReadCStr(ptr));
            },

            console_log: (ptr) => {
                console.log(wasmReadCStr(ptr));
            },

            request_animation_frame_loop: (f, ctx) => {
                function tick() {
                    if (wasm.exports.__indirect_function_table.get?.(f)(ctx)) {
                        requestAnimationFrame(tick);
                    }
                }
                requestAnimationFrame(tick);
            },

            canvas_get_by_id: (idPtr) => {
                const id = wasmReadCStr(idPtr);
                const el = document.getElementById(id);
                return (el && externRefGlue.insert(el)) || 0;
            },
            canvas_get_size: (elIdx, widthPtr, heightPtr) => {
                const el = externRefGlue.get(elIdx);
                wasmWriteI32(widthPtr, el.width);
                wasmWriteI32(heightPtr, el.height);
            },
            canvas_set_size: (elIdx, width, height) => {
                const el = externRefGlue.get(elIdx);
                el.width = width;
                el.height = height;
            },
            canvas_get_context: (elIdx, contextTypePtr) => {
                const el = externRefGlue.get(elIdx);
                const contextType = wasmReadCStr(contextTypePtr);
                const context = el.getContext(contextType);
                return (context && externRefGlue.insert(context)) || 0;
            },

            gl_clear_color: (ctxIdx, r, g, b, a) => {
                const ctx = externRefGlue.get(ctxIdx);
                ctx.clearColor(1.0, 0.0, 0.0, 1.0);
            },
            gl_clear: (ctxIdx, mask) => {
                const ctx = externRefGlue.get(ctxIdx);
                ctx.clear(mask);
            },
        },
    };

    const wasmStream = fetch(path);
    const wasmMod = await WebAssembly.compileStreaming(wasmStream);

    wasm = await WebAssembly.instantiate(wasmMod, imports);

    wasm.exports.main();

    window.__shin = {
        wasmMod,
        wasm,
        externRefGlue,
    };
}
