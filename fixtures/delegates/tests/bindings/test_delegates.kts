/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

import uniffi.delegates.*

object Delegate : MyDelegate {
    override fun <T> withReturn(thunk: () -> T): T = thunk().let { it }
    override fun <T> withoutReturn(thunk: () -> T) = thunk().let { Unit }
}