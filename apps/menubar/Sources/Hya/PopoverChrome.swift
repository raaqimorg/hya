import AppKit
import SwiftUI

/// Makes its hosting `NSWindow` borderless and transparent as soon as it's attached, so the
/// floating window `.menuBarExtraStyle(.window)` creates can show the rounded, vibrant card
/// `popoverChrome()` draws instead of a plain opaque rectangle. `viewDidMoveToWindow` (rather
/// than a dispatched closure) is what makes this reliable: the window is reused across every
/// open/close of the popover, and only this hook is guaranteed to fire exactly when the view
/// actually has a window to configure.
private final class ConfiguringView: NSView {
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let window else { return }
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = true
        if let contentView = window.contentView {
            contentView.wantsLayer = true
            contentView.layer?.backgroundColor = NSColor.clear.cgColor
        }
    }
}

private struct WindowAccessor: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        ConfiguringView()
    }

    func updateNSView(_ nsView: NSView, context: Context) {}
}

private struct PopoverChrome: ViewModifier {
    func body(content: Content) -> some View {
        content
            .background(.regularMaterial)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .strokeBorder(.separator, lineWidth: 0.5)
            )
            .background(WindowAccessor())
    }
}

extension View {
    /// Gives a `.menuBarExtraStyle(.window)` popover's content the look of a native floating
    /// panel: a vibrant material background, rounded corners, a hairline border, and a shadow
    /// from the now-transparent hosting window.
    func popoverChrome() -> some View {
        modifier(PopoverChrome())
    }
}
