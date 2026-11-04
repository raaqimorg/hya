import AppKit
import SwiftUI

/// Watches the hosting window's key status and calls `onResignKey` when it loses it: the user
/// clicked elsewhere to dismiss the popover, not through an in-app Cancel or Save. The window
/// `.menuBarExtraStyle(.window)` creates is reused across every open and close of the popover
/// (see `PopoverChrome.swift`), so `onDisappear` never fires here and cannot be used instead.
final class ResignKeyObservingView: NSView {
    var onResignKey: (() -> Void)?
    private var observer: NSObjectProtocol?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let observer {
            NotificationCenter.default.removeObserver(observer)
        }
        guard let window else { return }
        observer = NotificationCenter.default.addObserver(
            forName: NSWindow.didResignKeyNotification,
            object: window,
            queue: .main
        ) { [weak self] _ in self?.onResignKey?() }
    }
}

private struct ResignKeyObserver: NSViewRepresentable {
    let onResignKey: () -> Void

    func makeNSView(context: Context) -> NSView {
        let view = ResignKeyObservingView()
        view.onResignKey = onResignKey
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        (nsView as? ResignKeyObservingView)?.onResignKey = onResignKey
    }
}

extension View {
    /// Closes Settings when the popover is dismissed by clicking away, so reopening the menu
    /// bar item shows the schedule again instead of the Settings form it was left on.
    func resetSettingsOnDismiss(_ appState: AppState) -> some View {
        background(ResignKeyObserver { appState.settingsPresented = false })
    }
}
