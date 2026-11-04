import Combine
import Foundation

enum Mode: Equatable {
    case needsLocation
    case ready
    case error(HyaProcessError)
}

final class AppState: ObservableObject {
    @Published var mode: Mode
    @Published private(set) var locationResetToken = UUID()
    let scheduleController: ScheduleController

    private var cancellables: Set<AnyCancellable> = []

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

    func applyLocation(_ coordinates: Coordinates) {
        SettingsStore.shared.save(coordinates)
        scheduleController.stop()
        mode = .ready
        scheduleController.start(coordinates: coordinates)
    }

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

        scheduleController.objectWillChange.sink { [weak self] _ in
            self?.objectWillChange.send()
        }.store(in: &cancellables)
    }
}
