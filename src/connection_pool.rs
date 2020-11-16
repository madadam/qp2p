// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

// Pool for keeping open connections. Pooled connections are associated with a `ConnectionHandle`
// which can be used to remove them from the pool.
#[derive(Clone)]
pub(crate) struct ConnectionPool {
    store: Arc<Mutex<Store>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(Store::default())),
        }
    }

    pub fn insert(&self, addr: SocketAddr, conn: quinn::Connection) -> ConnectionHandle {
        // NOTE: this will panic if some other thread panicked while holding this lock. That's
        // probably the correct thing to do as it avoids working with potentially invalid state
        // (see the rust docs about mutex poisoning for more info).
        // That said, it's highly unlikely this will ever panic as none of the operations we
        // perform anywhere while holding the lock can panic.
        // This comment applies to all the other instances of `unwrap` in this module.
        let mut store = self.store.lock().unwrap();

        let key = Key {
            addr,
            id: store.id_gen.next(),
        };
        let _ = store.map.insert(key, conn);

        ConnectionHandle {
            store: self.store.clone(),
            key,
        }
    }

    pub fn get(&self, addr: &SocketAddr) -> Option<(quinn::Connection, ConnectionHandle)> {
        let mut store = self.store.lock().unwrap();

        // Efficiently fetch the first entry whose key is equal to `key`.
        let (key, conn) = store
            .map
            .range_mut(Key::min(*addr)..=Key::max(*addr))
            .next()?;

        let conn = conn.clone();
        let handle = ConnectionHandle {
            store: self.store.clone(),
            key: *key,
        };

        Some((conn, handle))
    }
}

// Handle to a connection in a pool which can be used to remove it.
#[derive(Clone)]
pub(crate) struct ConnectionHandle {
    store: Arc<Mutex<Store>>,
    key: Key,
}

impl ConnectionHandle {
    // Remove the connection from the pool.
    pub fn remove(&self) {
        let mut store = self.store.lock().unwrap();
        let _ = store.map.remove(&self.key);
    }

    pub fn remote_addr(&self) -> &SocketAddr {
        &self.key.addr
    }
}

#[derive(Default)]
struct Store {
    map: BTreeMap<Key, quinn::Connection>,
    id_gen: IdGen,
}

// Unique key identifying a connection. Two connections will always have distict keys even if they
// have the same socket address.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct Key {
    addr: SocketAddr,
    id: u64,
}

impl Key {
    // Returns the minimal `Key` for the given address according to its `Ord` relation.
    fn min(addr: SocketAddr) -> Self {
        Self { addr, id: u64::MIN }
    }

    // Returns the maximal `Key` for the given address according to its `Ord` relation.
    fn max(addr: SocketAddr) -> Self {
        Self { addr, id: u64::MAX }
    }
}

#[derive(Default)]
struct IdGen(u64);

impl IdGen {
    fn next(&mut self) -> u64 {
        let id = self.0;
        self.0 = self.0.wrapping_add(1);
        id
    }
}
