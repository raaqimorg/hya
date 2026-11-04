import Combine
import Foundation

enum Mode: Equatable {
    case needsLocation
    case ready
    case error(HyaProcessError)
}

final class AppState: ObservableObject {
    @Published var mode: Mode
    /// Bumped on every `resetLocation()`, so `HyaApp` can give `LocationHeaderView` a fresh
    /// `.id()` and force it to re-seed its fields from the now-cleared `SettingsStore` rather
    /// than keep showing the stale text it was initialized with.
    @Published private(set) var locationResetToken = UUID()
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

    /// Persists newly entered coordinates and restarts the schedule controller against them,
    /// whether this is the first location ever set or a later change. Also the target of the
    /// manual refresh button, so pressing it with unchanged coordinates is a harmless way to
    /// force a recompute right now.
    func applyLocation(_ coordinates: Coordinates) {
        SettingsStore.shared.save(coordinates)
        scheduleController.stop()
        mode = .ready
        scheduleController.start(coordinates: coordinates)
    }

    /// The target of the Reset button: clears the saved location and returns to the same
    /// first-run state a fresh install starts in.
    func resetLocation() {
        SettingsStore.shared.clearCoordinates()
        scheduleController.stop()
        mode = .needsLocation
        locationResetToken = UUID()
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
