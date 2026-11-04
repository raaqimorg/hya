import SwiftUI

@main
struct HyaApp: App {
    @StateObject private var appState = AppState(executableURL: AppState.bundledExecutableURL)

    init() {
        NSApplication.shared.setActivationPolicy(.accessory)
    }

    var body: some Scene {
        MenuBarExtra(label) {
            content
                .popoverChrome()
        }
        .menuBarExtraStyle(.window)
    }

    private var label: String {
        switch appState.mode {
        case .needsLocation:
            return "hya: Set location"
        case .ready:
            return appState.scheduleController.displayText
        case .error:
            return "hya: No times"
        }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 8) {
            LocationHeaderView(onApply: appState.applyLocation)
                .id(appState.locationResetToken)
            Divider()
            switch appState.mode {
            case .needsLocation:
                Text("Set your location above to see today's prayer times.")
                    .foregroundStyle(.secondary)
            case .ready:
                ScheduleListView(scheduleController: appState.scheduleController)
            case .error(let error):
                Text(description(for: error))
            }
            Divider()
            HStack {
                Button("Reset") { appState.resetLocation() }
                    .buttonStyle(.borderless)
                    .disabled(appState.mode == .needsLocation)
                Spacer()
                Button("Quit") { NSApplication.shared.terminate(nil) }
                    .buttonStyle(.borderless)
            }
        }
        .padding()
        .frame(minWidth: 260)
    }

    private func description(for error: HyaProcessError) -> String {
        switch error {
        case .notFound:
            return "The hya binary could not be found."
        case .nonZeroExit(let code):
            return "hya exited with status \(code)."
        case .invalidOutput:
            return "hya produced output this app could not read."
        }
    }
}
