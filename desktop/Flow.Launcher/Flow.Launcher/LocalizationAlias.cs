// The localization/API toolkit generates its namespace from the assembly identity,
// not RootNamespace. Keep Throw's executable identity while allowing the
// existing application source to resolve its own generated translations
// instead of the similarly named helpers in the engine libraries.
global using Localize = global::Throw.Launcher.Localize;
global using PublicApi = global::Throw.Launcher.PublicApi;
