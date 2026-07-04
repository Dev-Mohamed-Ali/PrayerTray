# Regenerates calc fixtures + the UmAlQura table from the C# implementation.
# Run after any change to Calc/ on the C# side, then copy outputs:
#   dotnet run -c Release --project rust/tools/genfix -- out
#   out/reference_times.json, out/hijri_fixture.json -> rust/tests/data/
#   out/umalqura_data.rs                             -> rust/src/calc/
