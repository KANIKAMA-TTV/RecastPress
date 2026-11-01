$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$inv = [System.Globalization.CultureInfo]::InvariantCulture
function F([double] $value) { $value.ToString('0.###', $inv) }

# Outline an ordinary font glyph before applying any subtractive treatment.
$family = [System.Drawing.FontFamily]::new('Times New Roman')
$glyph = [System.Drawing.Drawing2D.GraphicsPath]::new()
$glyph.AddString('R', $family, 0, 1000, [System.Drawing.PointF]::new(0, 0), [System.Drawing.StringFormat]::GenericTypographic)
$bounds = $glyph.GetBounds()
$scale = 600.0 / $bounds.Height
$left = 180.0
$top = 240.0
$points = $glyph.PathPoints
$types = $glyph.PathTypes
$outline = [System.Text.StringBuilder]::new()
function XY([int] $index) {
    $x = $left + ($points[$index].X - $bounds.X) * $scale
    $y = $top + ($points[$index].Y - $bounds.Y) * $scale
    "$(F $x) $(F $y)"
}
for ($i = 0; $i -lt $points.Length; $i++) {
    $kind = $types[$i] -band 7
    switch ($kind) {
        0 { [void] $outline.Append("M $(XY $i) ") }
        1 { [void] $outline.Append("L $(XY $i) ") }
        3 {
            if ($i + 2 -ge $points.Length) { throw 'Incomplete Bezier segment.' }
            [void] $outline.Append("C $(XY $i) $(XY ($i + 1)) $(XY ($i + 2)) ")
            $i += 2
        }
        default { throw "Unsupported path kind: $kind" }
    }
    if (($types[$i] -band 128) -ne 0) { [void] $outline.Append('Z ') }
}
$glyph.Dispose()
$family.Dispose()

# One shared circle cuts the entire letter, including the bowl.
$outerRadius = 450.0
$cutX = 500.0 - $outerRadius / [math]::Sqrt(2)
$cutY = $cutX
$cutRadius = 300.0
$cell = 10
$band = 40.0
$bayer = @(@(0, 8, 2, 10), @(12, 4, 14, 6), @(3, 11, 1, 9), @(15, 7, 13, 5))
$pixels = [System.Text.StringBuilder]::new()
$pixelCount = 0
for ($row = 0; $row -lt 100; $row++) {
    for ($col = 0; $col -lt 100; $col++) {
        $x = $col * $cell
        $y = $row * $cell
        $distance = [math]::Sqrt([math]::Pow($x + $cell / 2 - $cutX, 2) + [math]::Pow($y + $cell / 2 - $cutY, 2))
        if ([math]::Abs($distance - $cutRadius) -gt $band / 2) { continue }
        $coverage = ($distance - ($cutRadius - $band / 2)) / $band
        $threshold = ($bayer[$row % 4][$col % 4] + 0.5) / 16.0
        $color = if ($coverage -ge $threshold) { '#fff' } else { '#000' }
        [void] $pixels.AppendLine("<rect x=`"$x`" y=`"$y`" width=`"$cell`" height=`"$cell`" fill=`"$color`"/>")
        $pixelCount++
    }
}

$baseDefs = @"
<path id="normal-r" d="$outline" fill-rule="evenodd" stroke="#fff" stroke-width="6" stroke-linejoin="round"/>
<clipPath id="within-white-band"><circle cx="500" cy="500" r="423"/></clipPath>
<mask id="single-circular-cut" maskUnits="userSpaceOnUse" x="0" y="0" width="1000" height="1000" style="mask-type:luminance">
<rect width="1000" height="1000" fill="#fff"/>
<circle id="shared-cut-circle" cx="$(F $cutX)" cy="$(F $cutY)" r="$(F $cutRadius)" fill="#000"/>
<g id="pixel-boundary" shape-rendering="crispEdges">$pixels</g>
</mask>
"@
$frame = @'
<rect width="1000" height="1000" fill="#fff"/>
<circle cx="500" cy="500" r="450" fill="#24627a"/>
<circle cx="500" cy="500" r="423" fill="#fff"/>
<circle cx="500" cy="500" r="374.4" fill="#24627a"/>
'@
$stream = @'
<path id="continuous-injection" d="M 241.466 229.195 A 45 45 0 0 1 317.540 261.743 C 317.540 267.000 306.540 274.000 296.540 287.000 C 277.540 308.000 286.540 340.000 312.540 349.000 C 337.540 358.000 364.540 340.000 365.540 317.000 C 366.540 297.000 348.540 281.000 336.540 268.000 C 331.540 262.000 331.540 257.000 331.540 249.000 L 331.540 194.612 A 45 45 0 0 1 359.674 152.892 L 358.924 151.038 A 376.4 376.4 0 0 0 240.085 227.748 Z" fill="#fff"/>
'@
$letter = '<g clip-path="url(#within-white-band)"><use href="#normal-r" fill="#fff" mask="url(#single-circular-cut)"/></g>'
$uncutLetter = '<g clip-path="url(#within-white-band)"><use href="#normal-r" fill="#fff"/></g>'
$header = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000" width="1000" height="1000" role="img" aria-labelledby="title desc">'
$final = @"
$header
<title id="title">RecastPress logo concept 12</title>
<desc id="desc">A standard Roman serif R with both feet touching the circular band, a tapered hanging stream separated from the letter, and one circular cut finished with square pixels clipped to the original glyph.</desc>
<metadata>Typeface: Times New Roman Regular with a 6-unit outline weight increase. Cut center: $(F $cutX),$(F $cutY); radius: $(F $cutRadius). Center lies at the outer circumference at 10:30, 45 degrees counterclockwise from top. Pixel cell: $cell. Pixel band: $band. Boundary is a luminance mask on the original glyph; no white boundary pixels can exceed its fill.</metadata>
<defs>$baseDefs</defs>
$frame
$letter
$stream
</svg>
"@
$master = @"
$header
<title id="title">RecastPress normal R master 12</title>
<desc id="desc">The complete normal Roman serif glyph, enlarged and shifted left, before the circular cut. Both feet intersect the white circular band.</desc>
<defs>$baseDefs</defs>
$frame
$uncutLetter
</svg>
"@
Set-Content -LiteralPath (Join-Path $PSScriptRoot 'recastpress-logo-concept-v12.svg') -Value $final -Encoding utf8
Set-Content -LiteralPath (Join-Path $PSScriptRoot 'recastpress-logo-v12-uncut-master.svg') -Value $master -Encoding utf8
[pscustomobject]@{Typeface='Times New Roman Regular';GlyphWidth=($bounds.Width * $scale);GlyphHeight=600;GlyphLeft=$left;GlyphTop=$top;CutCenter=@($cutX,$cutY);CutRadius=$cutRadius;PixelCells=$pixelCount} | ConvertTo-Json -Compress










