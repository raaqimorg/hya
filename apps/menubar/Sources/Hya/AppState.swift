import Combine
import Foundation

enum Mode: Equatable {
    case needsLocation
    case ready
    case error(HyaProcessError)
}

final class AppState: ObservableObject {
    @Published var mode: Mode
    @Published var settingsPresented = false
    let scheduleController: ScheduleController

    private var cancellables: Set<AnyCancellable> = []

    /// The bundled `hya` CLI binary: a sibling of this app's own executable, placed there by
    /// `package-dmg.sh` under `Contents/MacOS/`. Named `hya-cli`, not `hya`: macOS's default
    /// case-insensitive-but-preserving APFS format treats `hya` and the app's own `Hya`
    /// executable as the same file, so a same-named sibling would silently overwrite it.
    static var bundledExecutableURL: URL {
        (Bundle.main.executableURL ?? URL(fileURLWithPath: CommandLine.arguments[0]))
            .deletingLastPathComponent()
            .appendingPathComponent("hya-cli")
    }

    init(executableURL: URL) {
        scheduleController = ScheduleController(executableURL: executableURL)

        if let coordinates = SettingsStore.shared.coordinates {
            mode = .ready
            observe()
            scheduleController.start(coordinates: coordinates)
        } else {
            mode = .needsLocation
            observe()
        }
    }

    /// Called after `SettingsView` has already saved the new coordinates to `SettingsStore`;
    /// restarts the controller against them.
    func settingsSaved() {
        guard let coordinates = SettingsStore.shared.coordinates else { return }
        scheduleController.stop()
        mode = .ready
        settingsPresented = false
        scheduleController.start(coordinates: coordinates)
    }

    private func observe() {
        scheduleController.$lastError.sink { [weak self] error in
            guard let self else { return }
            if let error {
                mode = .error(error)
            } else if case .error = mode {
                mode = .ready
            } else if SettingsStore.shared.coordinates != nil {
                mode = .ready
            }
        }.store(in: &cancellables)

        // Forwarded so the menu bar label (which observes only `AppState`, not the nested
        // `scheduleController`) re-renders on every minute tick, midnight refetch, and wake.
        scheduleController.objectWillChange.sink { [weak self] _ in
            self?.objectWillChange.send()
        }.store(in: &cancellables)
    }
}
