function assert(truth, msg) {
    if (!truth) {
        const baseMsg = "assertion failed";
        throw new Error(msg ? `${baseMsg}: ${msg}` : baseMsg);
    }
}

function assertEq(left, right, msg) {
    if (left !== right) {
        const baseMsg = `assertion left === right failed\n  left: ${left}\n right: ${right}`;
        throw new Error(msg ? `${baseMsg}: ${msg}` : baseMsg);
    }
}

function assertNe(left, right, msg) {
    if (left === right) {
        const baseMsg = `assertion left !== right failed\n  left: ${left}\n right: ${right}`;
        throw new Error(msg ? `${baseMsg}: ${msg}` : baseMsg);
    }
}

function todo(what) {
    const baseMsg = "not yet implemented"; 
    throw new Error(what ? `${baseMsg}: ${msg}` : baseMsg);
}

export class Glue {
    constructor() {
        this.instance = null;

        this.memoryViews = {};
        const getMemoryView = (type) => {
            const maybeMemoryView = this.memoryViews[type];
            const buffer = this.instance.exports.memory.buffer;
            if (!maybeMemoryView || maybeMemoryView.buffer !== buffer) {
                this.memoryViews[type] = new type(buffer);
            }
            return this.memoryViews[type];
        }

        // NOTE: predefined values. must be kept up to date with consts in
        // rust.
        const UNDEFINED = 0;
        const NULL = 1;
        const GLOBAL = 2;
        const GLUE = 3;
        this.handleTable = new WebAssembly.Table({ element: "externref", initial: 0 });
        // like Rc. set to 1 on alloc, inc by 1 on clone, dec by 1 on drop.
        // when reaches 0 the handle is pushed into the free-list.
        this.handleCounts = [];
        this.freeHandles = [];
        // stores value, returns handle.
        const allocValueUnchecked = (value) => {
            let maybeHandle = this.freeHandles.pop();
            if (maybeHandle === undefined) {
                assertEq(this.freeHandles.length, 0);

                const oldLen = this.handleTable.length;
                const growDelta = Math.max(1, oldLen);
                this.handleTable.grow(growDelta);
                // NOTE: iterate in reverse order to push smaller indices to
                // the end.
                for (let i = growDelta; i > 0; i -= 1) {
                    this.freeHandles.push(oldLen + i - 1);
                }

                assertEq(this.handleTable.length, oldLen + growDelta);
                assertEq(this.freeHandles.length, growDelta);

                maybeHandle = this.freeHandles.pop();
            }
            assertNe(maybeHandle, undefined);
            const handle = maybeHandle;

            this.handleTable.set(handle, value);
            this.handleCounts[handle] = 1;
            return handle;
        };
        // me being pedantic.
        assertEq(allocValueUnchecked(undefined), UNDEFINED);
        assertEq(allocValueUnchecked(null), NULL);
        assertEq(allocValueUnchecked(globalThis), GLOBAL);
        assertEq(allocValueUnchecked(this), GLUE);
        const allocValue = (value) => {
            // NOTE: i kind of want to avoid a need for nil checks
            // everywhere.
            if (value === undefined) {
                this.handleCounts[UNDEFINED] += 1;
                return UNDEFINED;
            }
            if (value === null) {
                this.handleCounts[NULL] += 1;
                return NULL;
            }
            // TODO: predefine true and false.
            
            return allocValueUnchecked(value);
        };
        const getValue = (handle) => {
            assertEq(typeof handle, "number");
            return this.handleTable.get(handle);
        };

        this.textDecoder = new TextDecoder();
        const decodeString = (ptr, len) => {
            const slice = getMemoryView(Uint8Array).subarray(ptr, ptr + len);
            return this.textDecoder.decode(slice);
        };

        this.textEncoder = new TextEncoder();
        const encodeString = (s) => {
            return this.textEncoder.encode(s);
        };

        this.importObject = {
            env: {
                throw_str: (ptr, len) => {
                    const msg = decodeString(ptr, len);
                    throw new Error(msg);
                },

                string_new: (ptr, len) => {
                    const string = decodeString(ptr, len);
                    return allocValue(string);
                },
                number_new: (value) => {
                    return allocValue(value);
                },
                closure_new: (callByPtrIndex, ptr) => {
                    const callByPtr = this.instance.exports.
                        __indirect_function_table.get(callByPtrIndex);
                    const callByPtrWrapped = (...args) => {
                        // TODO: handle args
                        // const argHandles = args.map(allocValue);
                        callByPtr(ptr);
                    };
                    return allocValue(callByPtrWrapped);
                },

                increment_strong_count: (handle) => {
                    this.handleCounts[handle] += 1;
                },
                decrement_strong_count: (handle) => {
                    this.handleCounts[handle] -= 1;
                    const count = this.handleCounts[handle];
                    assert(count >= 0);
                    if (count == 0) {
                        this.freeHandles.push(handle);
                    }
                },

                is_object: (handle) => {
                    const value = getValue(handle);
                    // NOTE: null is an object too kekw, see
                    // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/typeof#typeof_null
                    return typeof value === "object" && value !== null;
                },
                is_function: (handle) => {
                    return typeof getValue(handle) === "function";
                },
                is_number: (handle) => {
                    return typeof getValue(handle) === "number";
                },
                is_string: (handle) => {
                    return typeof getValue(handle) === "string";
                },

                get: (handle, propPtr, propLen) => {
                    const target = getValue(handle);
                    const prop = decodeString(propPtr, propLen);
                    let value = Reflect.get(target, prop);
                    if (value instanceof Function) {
                        value = value.bind(target);
                    }
                    return allocValue(value);
                },
                set: (handle, propPtr, propLen, valueHandle) => {
                    const target = getValue(handle);
                    const prop = decodeString(propPtr, propLen);
                    const value = getValue(valueHandle);
                    Reflect.set(target, prop, value);
                },
                call: (handle, argsPtr, argsLen, retHandlePtr) => {
                    const mem = getMemoryView(DataView);
                    try {
                        const target = getValue(handle);
                        const args = Array.from({ length: argsLen }, (_, i) => {
                            return getValue(mem.getUint32(argsPtr + i * 4, true));
                        });
                        const result = Reflect.apply(target, undefined, args);
                        mem.setUint32(retHandlePtr, allocValue(result), true);
                        return true;
                    } catch (err) {
                        mem.setUint32(retHandlePtr, allocValue(err), true);
                        return false;
                    }
                },

                number_get: (handle) => {
                    const value = getValue(handle);
                    assert(typeof value, "number");
                    return value;
                },
                string_get: (handle, ptrPtr, lenPtr) => {
                    const value = getValue(handle);
                    assert(typeof value, "string");

                    const buf = encodeString(value);
                    const ptr = this.instance.exports.alloc(buf.length, 1);
                    getMemoryView(Uint8Array).subarray(ptr, ptr + buf.length).set(buf);

                    const mem = getMemoryView(DataView);
                    mem.setUint32(ptrPtr, ptr, true);
                    mem.setUint32(lenPtr, buf.length, true);
                }
            },
        };
    }

    init(instance) {
        assert(instance instanceof WebAssembly.Instance);
        assert(!this.instance);
        this.instance = instance;
    }
}

