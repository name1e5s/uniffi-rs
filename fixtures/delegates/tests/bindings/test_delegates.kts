/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

import uniffi.delegates.*

/// MyDelegate is defined in the UDL file. It generates an interface.
/// Implementations of the interface never cross the FFI boundary, and so
/// can contain arbitrary Kotlin.
class Delegate : MyDelegate {
    var count = 0
    var lastString: String? = null

    override fun <T> withReturn(thunk: () -> T): T = thunk()
    override fun <T> stringSaver(thunk: () -> T) {
        lastString = thunk() as? String
    }
    override fun <T> withCounter(thunk: () -> T): Int = thunk().let { ++count }
}

val delegate = Delegate()
// Delegates are given to the rust object via a constructor.
val rustObj0 = RustObject(delegate)

val string0 = "placeholder string"
// Alternative constructors take the delegate as the first argument.
// The argument name is a mixed case version of the delegate interface name.
val rustObj1 = RustObject.fromString(string = string0, myDelegate = delegate)

assert(rustObj0.length() == string0.length) { "generic return" }

assert(rustObj0.getString() == 1) { "different return type from method's own" }
assert(rustObj0.getString() == 2) { "code is run each time the method is run" }
assert(rustObj1.getString() == 3) { "delegate can be shared between objects" }

val string1 = "meta-syntactic variable values"
assert(rustObj1.identityString(string1) == Unit) { "void return" }
assert(delegate.lastString == string1) { "" }