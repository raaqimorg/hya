import Foundation

struct Coordinates: Codable, Equatable {
    let latitude: Double
    let longitude: Double

    enum ValidationError: Error {
        case notFinite
        case latitudeOutOfRange
        case longitudeOutOfRange
    }

    static func validated(latitude: String, longitude: String) -> Result<Coordinates, ValidationError> {
        guard let lat = Double(latitude), let lng = Double(longitude), lat.isFinite, lng.isFinite else {
            return .failure(.notFinite)
        }
        guard (-90...90).contains(lat) else {
            return .failure(.latitudeOutOfRange)
        }
        guard (-180...180).contains(lng) else {
            return .failure(.longitudeOutOfRange)
        }
        return .success(Coordinates(latitude: lat, longitude: lng))
    }
}

final class SettingsStore: ObservableObject {
    @Published private(set) var coordinates: Coordinates?

    private let defaults: UserDefaults
    private static let latitudeKey = "latitude"
    private static let longitudeKey = "longitude"

    static let shared = SettingsStore(defaults: .standard)

    init(defaults: UserDefaults) {
        self.defaults = defaults
        if defaults.object(forKey: Self.latitudeKey) != nil, defaults.object(forKey: Self.longitudeKey) != nil {
            coordinates = Coordinates(
                latitude: defaults.double(forKey: Self.latitudeKey),
                longitude: defaults.double(forKey: Self.longitudeKey)
            )
        }
    }

    func save(_ coordinates: Coordinates) {
        defaults.set(coordinates.latitude, forKey: Self.latitudeKey)
        defaults.set(coordinates.longitude, forKey: Self.longitudeKey)
        self.coordinates = coordinates
    }
}
