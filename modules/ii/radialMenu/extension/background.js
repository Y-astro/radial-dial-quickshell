// Radial Dial Real-Time Tab Synchronizer
let port = null;

function connectNative() {
    try {
        port = browser.runtime.connectNative("radial_tabs");
        port.onDisconnect.addListener((p) => {
            console.warn("[RadialDial] Native port disconnected:", p.error);
            port = null;
            // Retry connecting after 3 seconds
            setTimeout(connectNative, 3000);
        });
        syncTabs();
    } catch (e) {
        console.error("[RadialDial] Failed to connect to native host:", e);
        setTimeout(connectNative, 3000);
    }
}

function syncTabs() {
    browser.windows.getLastFocused({ populate: true }).then((win) => {
        if (!win || !win.tabs) return;
        const tabs = win.tabs.map((t) => ({
            index: t.index + 1,
            title: t.title || `Tab ${t.index + 1}`,
            url: t.url || "",
            active: t.active,
            id: t.id
        }));
        if (port) {
            port.postMessage(tabs);
        }
    }).catch(() => {
        browser.tabs.query({ currentWindow: true }).then((tabs) => {
            if (!tabs) return;
            const mapped = tabs.map((t) => ({
                index: t.index + 1,
                title: t.title || `Tab ${t.index + 1}`,
                url: t.url || "",
                active: t.active,
                id: t.id
            }));
            if (port) {
                port.postMessage(mapped);
            }
        }).catch(() => {});
    });
}

// Connect immediately
connectNative();

// Event Listeners
browser.tabs.onCreated.addListener(() => syncTabs());
browser.tabs.onRemoved.addListener(() => syncTabs());
browser.tabs.onUpdated.addListener((id, change) => {
    if (change.title || change.url || change.status === "complete") {
        syncTabs();
    }
});
browser.tabs.onActivated.addListener(() => syncTabs());
browser.tabs.onMoved.addListener(() => syncTabs());
browser.tabs.onAttached.addListener(() => syncTabs());
browser.tabs.onDetached.addListener(() => syncTabs());
browser.windows.onFocusChanged.addListener(() => syncTabs());
