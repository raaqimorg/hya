import Foundation
import AppKit

/// The next local midnight strictly after `now`. Always succeeds: a calendar always has a
/// next midnight, so the force-unwrap can't fail.
func nextLocalMidnight(after now: Date, calendar: Calendar = .current) -> Date {
    calendar.nextDate(
        after: now,
        matching: DateComponents(hour: 0, minute: 0, second: 0),
        matchingPolicy: .nextTime
    )!
}

final class ScheduleController: ObservableObject {
    @Published private(set) var displayText: String = "hya"
    @Published private(set) var snapshot: PrayerSnapshot?
    @Published private(set) var lastError: HyaProcessError?

    private let executableURL: URL
    private var coordinates: Coordinates?
    private var minuteTimer: Timer?
    private var midnightTimer: Timer?
    private var wakeObserver: NSObjectProtocol?

    init(executableURL: URL) {
        self.executableURL = executableURL
    }

    func start(coordinates: Coordinates) {
        self.coordinates = coordinates
        refetch()
        scheduleMinuteTimer()
        scheduleMidnightTimer()
        wakeObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didWakeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.recomputeDisplayText()
        }
    }

    func stop() {
        minuteTimer?.invalidate()
        minuteTimer = nil
        midnightTimer?.invalidate()
        midnightTimer = nil
        if let wakeObserver {
            NSWorkspace.shared.notificationCenter.removeObserver(wakeObserver)
        }
        wakeObserver = nil
    }

    private func refetch() {
        guard let coordinates else { return }
        switch runHya(executableURL: executableURL, coordinates: coordinates) {
        case .success(let snapshot):
            self.snapshot = snapshot
            lastError = nil
            recomputeDisplayText()
        case .failure(let error):
            lastError = error
            displayText = "hya: No times"
        }
    }

    private func recomputeDisplayText() {
        guard let snapshot else { return }
        displayText = statusText(for: snapshot, at: Date())
    }

    private func scheduleMinuteTimer() {
        let next = nextMinuteBoundary(after: Date())
        minuteTimer = Timer(fire: next, interval: 60, repeats: true) { [weak self] _ in
            self?.recomputeDisplayText()
        }
        if let minuteTimer {
            RunLoop.main.add(minuteTimer, forMode: .common)
        }
    }

    private func scheduleMidnightTimer() {
        let next = nextLocalMidnight(after: Date())
        midnightTimer = Timer(fire: next, interval: 0, repeats: false) { [weak self] _ in
            self?.refetch()
            self?.scheduleMidnightTimer()
        }
        if let midnightTimer {
            RunLoop.main.add(midnightTimer, forMode: .common)
        }
    }

    private func nextMinuteBoundary(after now: Date, calendar: Calendar = .current) -> Date {
        calendar.nextDate(
            after: now,
            matching: DateComponents(second: 0),
            matchingPolicy: .nextTime
        ) ?? now.addingTimeInterval(60)
    }
}
