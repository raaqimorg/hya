import Foundation

enum HyaProcessError: Error, Equatable {
    case notFound
    case nonZeroExit(Int32)
    case invalidOutput
}

func runHya(executableURL: URL, coordinates: Coordinates) -> Result<PrayerSnapshot, HyaProcessError> {
    let process = Process()
    process.executableURL = executableURL
    process.arguments = ["--json", "--lat", "\(coordinates.latitude)", "--lng", "\(coordinates.longitude)"]

    let stdout = Pipe()
    process.standardOutput = stdout
    process.standardError = Pipe()

    do {
        try process.run()
    } catch {
        return .failure(.notFound)
    }
    process.waitUntilExit()

    guard process.terminationStatus == 0 else {
        return .failure(.nonZeroExit(process.terminationStatus))
    }

    let data = stdout.fileHandleForReading.readDataToEndOfFile()
    guard let snapshot = try? JSONDecoder().decode(PrayerSnapshot.self, from: data) else {
        return .failure(.invalidOutput)
    }
    return .success(snapshot)
}
