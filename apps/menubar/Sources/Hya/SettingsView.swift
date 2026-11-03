import SwiftUI

struct SettingsView: View {
    @State private var latitude = ""
    @State private var longitude = ""

    let onSave: () -> Void

    private var validated: Coordinates? {
        try? Coordinates.validated(latitude: latitude, longitude: longitude).get()
    }

    var body: some View {
        Form {
            TextField("Latitude", text: $latitude)
            TextField("Longitude", text: $longitude)
            Button("Save") {
                guard let coordinates = validated else { return }
                SettingsStore.shared.save(coordinates)
                onSave()
            }
            .disabled(validated == nil)
        }
        .padding()
    }
}
