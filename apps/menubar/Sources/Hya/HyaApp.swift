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
                .resetSettingsOnDismiss(appState)
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

    @ViewBuilder
    private var content: some View {
        if appState.mode == .needsLocation {
            SettingsView(onSave: { appState.settingsSaved() }, onCancel: nil)
        } else if appState.settingsPresented {
            SettingsView(
                onSave: { appState.settingsSaved() },
                onCancel: { appState.settingsPresented = false }
            )
        } else {
            switch appState.mode {
            case .needsLocation:
                EmptyView()
            case .ready:
                ScheduleListView(
                    scheduleController: appState.scheduleController,
                    onSettings: { appState.settingsPresented = true }
                )
            case .error(let error):
                VStack(alignment: .leading, spacing: 4) {
                    Text(description(for: error))
                    Divider()
                    HStack {
                        Button("Settings…") { appState.settingsPresented = true }
                        Spacer()
                        Button("Quit") { NSApplication.shared.terminate(nil) }
                    }
                }
                .padding()
            }
        }
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
