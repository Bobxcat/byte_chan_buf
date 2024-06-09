use std::{
    error::Error,
    fmt::{Debug, Display},
    io::{Read, Write},
    mem,
    ops::DerefMut,
    sync::{Arc, Weak},
};

use parking_lot::Mutex;

pub fn byte_chan() -> (ByteTx, ByteRx) {
    let buf = Buf::new_arc();
    let tx_sig = ActiveSig::new();
    let tx_watcher = tx_sig.watcher();

    let rx_sig = ActiveSig::new();
    let rx_watcher = tx_sig.watcher();

    (
        ByteTx {
            buf: buf.clone(),
            _tx_active: tx_sig,
            rx_active: rx_watcher,
        },
        ByteRx {
            buf,
            tx_active: tx_watcher,
            _rx_active: rx_sig,
            curr: vec![].into_boxed_slice(),
            cursor: 0,
        },
    )
}

#[derive(Debug, Default)]
struct Buf {
    inner: Mutex<Vec<u8>>,
}

impl Buf {
    pub fn new_arc() -> Arc<Self> {
        Arc::default()
    }

    pub fn take(&self) -> Vec<u8> {
        let mut v = self.lock_mut();
        mem::replace(&mut v, vec![])
    }

    pub fn lock_mut<'a>(&'a self) -> impl DerefMut<Target = Vec<u8>> + 'a {
        self.inner.lock()
    }
}

#[derive(Debug, Clone)]
struct ActiveSig {
    inner: Arc<()>,
}

impl ActiveSig {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(()),
        }
    }
    pub fn watcher(&self) -> ActiveWatcher {
        ActiveWatcher {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveWatcher {
    inner: Weak<()>,
}

impl ActiveWatcher {
    pub fn sig_open(&self) -> bool {
        self.inner.upgrade().is_some()
    }

    pub fn sig_closed(&self) -> bool {
        !self.sig_open()
    }
}

#[derive(Debug, Clone)]
pub struct ByteTx {
    buf: Arc<Buf>,
    _tx_active: ActiveSig,
    rx_active: ActiveWatcher,
}

impl ByteTx {
    pub fn rx_closed(&self) -> bool {
        self.rx_active.sig_closed()
    }
    pub fn send(&self, b: u8) {
        self.buf.lock_mut().push(b)
    }
}

impl Write for ByteTx {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.lock_mut().write(buf)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum RxErr {
    TxClosed,
}

impl Display for RxErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

impl Error for RxErr {}

#[derive(Debug)]
pub struct ByteRx {
    buf: Arc<Buf>,
    _rx_active: ActiveSig,
    tx_active: ActiveWatcher,

    curr: Box<[u8]>,
    cursor: usize,
}

impl ByteRx {
    pub fn tx_closed(&self) -> bool {
        self.tx_active.sig_closed()
    }

    fn next_chunk(&mut self) -> Result<(), RxErr> {
        loop {
            let mut new = self.buf.take();
            if new.is_empty() {
                if self.tx_closed() {
                    // We have to check again to make sure any writes that happened between
                    // checking the buffer initially and dropping the tx activator are flushed
                    new = self.buf.take();
                    if new.is_empty() {
                        return Err(RxErr::TxClosed);
                    }
                } else {
                    std::hint::spin_loop();
                    continue;
                }
            }

            self.curr = new.into_boxed_slice();
            self.cursor = 0;
            return Ok(());
        }
    }

    pub fn recv(&mut self) -> Result<u8, RxErr> {
        if self.cursor >= self.curr.len() {
            self.next_chunk()?;
        }

        let b = self.curr[self.cursor];
        self.cursor += 1;
        Ok(b)
    }
}

impl Read for ByteRx {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cursor >= self.curr.len() {
            match self.next_chunk() {
                Ok(()) => (),
                Err(RxErr::TxClosed) => return Ok(0),
            }
        }

        let mut sl = &self.curr[self.cursor..];
        let bytes_read = sl.read(buf)?;
        self.cursor += bytes_read;
        Ok(bytes_read)
    }
}
