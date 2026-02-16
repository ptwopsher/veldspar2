use std::sync::mpsc;

pub struct EventSender<T> {
    tx: mpsc::Sender<T>,
}

pub struct EventReceiver<T> {
    rx: mpsc::Receiver<T>,
}

pub fn channel<T>() -> (EventSender<T>, EventReceiver<T>) {
    let (tx, rx) = mpsc::channel();
    (EventSender { tx }, EventReceiver { rx })
}

impl<T> Clone for EventSender<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

impl<T> EventSender<T> {
    pub fn send(&self, event: T) -> Result<(), mpsc::SendError<T>> {
        self.tx.send(event)
    }
}

impl<T> EventReceiver<T> {
    pub fn recv(&self) -> Result<T, mpsc::RecvError> {
        self.rx.recv()
    }

    pub fn try_recv(&self) -> Result<T, mpsc::TryRecvError> {
        self.rx.try_recv()
    }

    pub fn iter(&self) -> mpsc::Iter<'_, T> {
        self.rx.iter()
    }
}
