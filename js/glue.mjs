// TODO: consider introducing some kind of debug mode with setDebug method?

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

// NOTE: consts below must be kept in sync with rust.

const QUIET_NAN = 0x7ff8_0000_0000_0000n;
const TY_MASK = (1n << 8n) - 1n;
const ID_MASK = (1n << 32n) - 1n;

const TY_DONT_CARE = 0;
const TY_OBJECT = 1;
const TY_FUNCTION = 2;
const TY_STRING = 3;

const ID_UNDEFINED = 1;
const ID_NULL = 2;
const ID_NAN = 3;
const ID_TRUE = 4;
const ID_FALSE = 5;
const ID_GLOBAL = 6;
const ID_GLUE = 7;
const ID_MAX = 8;

function rsValueFromTyId(ty, id) {
    assertEq(typeof ty, "number");
    assertEq(typeof id, "number");
    return (QUIET_NAN | (BigInt(ty) << 32n) | BigInt(id));
}

function rsValueTy(value) {
    assertEq(typeof value, "bigint");
    return (value >> 32n) & TY_MASK;
}

function rsValueId(value) {
    assertEq(typeof value, "bigint");
    return value & ID_MASK;
}

function rsValueIsPredefined(value) {
    assertEq(typeof value, "bigint");
    return rsValueId(value) < ID_MAX;
}

const UNDEFINED = rsValueFromTyId(TY_DONT_CARE, ID_UNDEFINED);
const NULL = rsValueFromTyId(TY_DONT_CARE, ID_NULL);
const NAN = rsValueFromTyId(TY_DONT_CARE, ID_NAN);
const TRUE = rsValueFromTyId(TY_DONT_CARE, ID_TRUE);
const FALSE = rsValueFromTyId(TY_DONT_CARE, ID_FALSE);
const GLOBAL = rsValueFromTyId(TY_OBJECT, ID_GLOBAL);
const GLUE = rsValueFromTyId(TY_OBJECT, ID_GLUE);

function rsValueIdx(value) {
    assertEq(typeof value, "bigint");
    assert(!rsValueIsPredefined(value));
    return Number(rsValueId(value)) - ID_MAX;
}

function rsValueFromTyIdx(ty, idx) {
    assertEq(typeof ty, "number");
    assertEq(typeof idx, "number");
    return rsValueFromTyId(ty, idx + ID_MAX);
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

        this.values = new WebAssembly.Table({ element: "externref", initial: 0 });
        // set ref count to 1 on alloc, inc by 1 on clone, dec by 1 on drop.
        // when count reaches 0 the handle is pushed into the free-list and
        // value must be set to undefined.
        this.refCounts = [];
        this.freeIndices = [];
        // allocValue returns index.
        const allocValue = (value) => {
            let maybeIdx = this.freeIndices.pop();
            if (maybeIdx === undefined) {
                assertEq(this.freeIndices.length, 0);
                const oldLen = this.values.length;
                const delta = Math.max(1, oldLen);
                this.values.grow(delta);
                for (let i = 0; i < delta; i += 1) {
                    this.freeIndices.push(oldLen + i);
                }
                assertEq(this.values.length, oldLen + delta);
                assertEq(this.freeIndices.length, delta);
                maybeIdx = this.freeIndices.pop();
            }
            assertNe(maybeIdx, undefined);
            const idx = maybeIdx;
            this.values.set(idx, value);
            this.refCounts[idx] = 1;
            return idx;
        };
        const storeJsValueIntoRsValuePtr = (jsValue, rsValuePtr) => {
            assertEq(typeof rsValuePtr, "number");
            const mem = getMemoryView(DataView);
            switch (jsValue) {
                case undefined: return mem.setBigUint64(rsValuePtr, UNDEFINED, true);
                case null: return mem.setBigUint64(rsValuePtr, NULL, true);
                case true: return mem.setBigUint64(rsValuePtr, TRUE, true);
                case false: return mem.setBigUint64(rsValuePtr, FALSE, true);
                case globalThis: return assert(false);
                case this: return assert(false);
            }
            if (typeof jsValue === "number") {
                // NOTE: nan is a special case that cannot be covered by the switch
                // statement above as NaN !== NaN.
                // also note that isNan and Number.isNaN behave differently.
                if (Number.isNaN(jsValue)) {
                    mem.setBigUint64(rsValuePtr, NAN, true);
                    return;
                }
                mem.setFloat64(rsValuePtr, jsValue, true);
                return;
            }
            let ty = TY_DONT_CARE;
            switch (typeof jsValue) {
                case "object":
                    // NOTE: typeof null === "object"; but that doesn't affect
                    // us here because null is covered above ^.
                    assertNe(jsValue, null);
                    ty = TY_OBJECT;
                    break;
                case "function":
                    ty = TY_FUNCTION;
                    break;
                case "string":
                    ty = TY_STRING;
                    break;
            }
            const idx = allocValue(jsValue);
            const rsValue = rsValueFromTyIdx(ty, idx);
            mem.setBigUint64(rsValuePtr, rsValue, true);
        };
        const resolveJsValueFromRsValue = (rsValue) => {
            assertEq(typeof rsValue, "bigint");
            switch (rsValue) {
                case UNDEFINED: return undefined;
                case NULL: return null;
                case NAN: return NaN;
                case TRUE: return true;
                case FALSE: return false;
                case GLOBAL: return globalThis;
                case GLUE: return this;
            }
            const idx = rsValueIdx(rsValue);
            return this.values.get(idx);
        };
        const resolveJsValueFromRsValuePtr = (rsValuePtr) => {
            assertEq(typeof rsValuePtr, "number");
            const mem = getMemoryView(DataView);
            const f = mem.getFloat64(rsValuePtr, true);
            if (!Number.isNaN(f)) {
                return f;
            }
            const rsValue = mem.getBigUint64(rsValuePtr, true);
            return resolveJsValueFromRsValue(rsValue);
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

                string_new: (ptr, len, outPtr) => {
                    const value = decodeString(ptr, len);
                    storeJsValueIntoRsValuePtr(value, outPtr);
                },
                closure_new: (callByPtrIdx, ptr, outPtr) => {
                    const callByPtr = this.instance.exports.
                        __indirect_function_table.get(callByPtrIdx);
                    // TODO: is wrapper really needed? can this be avoided?
                    const callByPtrWrapped = (...args) => {
                        // TODO: args
                        callByPtr(ptr);
                    };
                    storeJsValueIntoRsValuePtr(callByPtrWrapped, outPtr);
                },

                increment_ref_count: (ref) => {
                    const idx = rsValueIdx(ref);
                    this.refCounts[idx] += 1;
                },
                decrement_ref_count: (ref) => {
                    const idx = rsValueIdx(ref);
                    this.refCounts[idx] -= 1;
                    const count = this.refCounts[idx];
                    assert(count >= 0);
                    if (count === 0) {
                        this.freeIndices.push(idx);
                        // to sort of let js know that it can be gc'd.
                        this.values.set(idx, undefined);
                    }
                },

                get: (ref, propPtr, propLen, outPtr) => {
                    const target = resolveJsValueFromRsValue(ref);
                    const prop = decodeString(propPtr, propLen);
                    let value = Reflect.get(target, prop);
                    // NOTE: if this is a function we want to be able to call it
                    // without having to also pass around the object that owns
                    // the method or whatever is the correct terminology for
                    // this.
                    if (value instanceof Function) {
                        value = value.bind(target);
                    }
                    storeJsValueIntoRsValuePtr(value, outPtr);
                },
                set: (ref, propPtr, propLen, valuePtr) => {
                    const target = resolveJsValueFromRsValue(ref);
                    const prop = decodeString(propPtr, propLen);
                    const value = resolveJsValueFromRsValuePtr(valuePtr);
                    Reflect.set(target, prop, value);
                },

                call: (ref, argsPtr, argsLen, outPtr) => {
                    const mem = getMemoryView(DataView);
                    try {
                        const target = resolveJsValueFromRsValue(ref);
                        const args = Array.from({ length: argsLen }, (_, i) => {
                            return resolveJsValueFromRsValuePtr(argsPtr + i * 8);
                        });
                        const ok = Reflect.apply(target, undefined, args);
                        storeJsValueIntoRsValuePtr(ok, outPtr);
                        return true;
                    } catch (err) {
                        storeJsValueIntoRsValuePtr(err, outPtr);
                        return false;
                    }
                },
                construct: (ref, argsPtr, argsLen, outPtr) => {
                    const mem = getMemoryView(DataView);
                    try {
                        const target = resolveJsValueFromRsValue(ref);
                        const args = Array.from({ length: argsLen }, (_, i) => {
                            return resolveJsValueFromRsValuePtr(argsPtr + i * 8);
                        });
                        const ok = Reflect.construct(target, args);
                        storeJsValueIntoRsValuePtr(ok, outPtr);
                        return true;
                    } catch (err) {
                        storeJsValueIntoRsValuePtr(err, outPtr);
                        return false;
                    }
                },


                string_get: (ref, ptrPtr, lenPtr) => {
                    const value = resolveJsValueFromRsValue(ref);
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

        // NOTE: uncomment code below to log all calls that rust makes.
        //
        // Object.keys(this.importObject.env).forEach((key) => {
        //     const value = this.importObject.env[key];
        //     if (typeof value !== "function") {
        //         return;
        //     }
        //     this.importObject.env[key] = (...args) => {
        //         const ret = value(...args);
        //         console.log(`${key.padEnd(32)} (${args.join(", ").padEnd(64)}) -> ${ret}`);
        //         return ret;
        //     };
        // });
    }

    init = (instance) => {
        assert(instance instanceof WebAssembly.Instance);
        assert(!this.instance);
        this.instance = instance;
    }
}

