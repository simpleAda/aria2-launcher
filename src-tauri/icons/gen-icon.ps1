# 二次元 ICO 生成器 v2：粉紫渐变圆底 + 樱花 + 白色下载箭头 + 星芒
# 仅使用 SolidBrush + FillPolygon/FillRectangle/FillEllipse（不用 Pen 构造器）
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$ErrorActionPreference = 'Stop'

$scriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }
$outIco = Join-Path $scriptDir 'icon.ico'
$outPng = Join-Path $scriptDir 'icon-preview.png'

function New-IconBitmap([int]$size) {
  $bmp = New-Object System.Drawing.Bitmap($size, $size)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'
  $u = $size / 256.0

  # ---------- 1) 粉紫渐变圆底 ----------
  $r = [System.Drawing.RectangleF]::new(2*$u, 2*$u, $size-4*$u, $size-4*$u)
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $path.AddEllipse($r)
  $gb = New-Object System.Drawing.Drawing2D.LinearGradientBrush($r,
    [System.Drawing.Color]::FromArgb(255, 244, 143, 205),
    [System.Drawing.Color]::FromArgb(255, 130, 105, 235), 45.0)
  $g.FillPath($gb, $path)
  $gb.Dispose(); $path.Dispose()

  # ---------- 2) 樱花（五瓣） ----------
  $p1 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(235, 255, 235, 246))
  $p2 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(200, 255, 235, 246))
  $c1 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 214, 130))
  $c2 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 224, 150))

  function Draw-Flower([System.Drawing.Graphics]$gg, [double]$cx, [double]$cy,
                       [double]$rad, [double]$rot, $petal, $core) {
    $st = $gg.Transform.Clone()
    $gg.TranslateTransform($cx, $cy); $gg.RotateTransform($rot)
    for ($i = 0; $i -lt 5; $i++) {
      $a = $i * 72
      $gg.RotateTransform($a); $gg.TranslateTransform(0, -$rad*0.55)
      $gg.FillEllipse($petal, -$rad*0.275, -$rad*0.425, $rad*0.55, $rad*0.85)
      $gg.TranslateTransform(0, $rad*0.55); $gg.RotateTransform(-$a)
    }
    $gg.FillEllipse($core, -$rad*0.18, -$rad*0.18, $rad*0.36, $rad*0.36)
    $gg.Transform = $st; $st.Dispose()
  }
  Draw-Flower $g (56*$u) (56*$u) (30*$u) 15 $p1 $c1
  Draw-Flower $g (206*$u) (44*$u) (20*$u) -20 $p2 $c2
  $p1.Dispose(); $p2.Dispose(); $c1.Dispose(); $c2.Dispose()

  # ---------- 3) 白色下载箭头 + 托盘（全用填充） ----------
  $w = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
  $w2 = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(235, 255, 255, 255))

  $hw = 10*$u
  $g.FillEllipse($w, (128*$u-$hw), (76*$u), (2*$hw), (2*$hw))
  $g.FillRectangle($w, (128*$u-$hw), (86*$u), (2*$hw), (54*$u))
  $tri = @(
    [System.Drawing.PointF]::new((84*$u), (136*$u)),
    [System.Drawing.PointF]::new((172*$u), (136*$u)),
    [System.Drawing.PointF]::new((128*$u), (188*$u))
  )
  $g.FillPolygon($w, $tri)
  $g.FillRectangle($w2, (88*$u), (202*$u), (80*$u), (12*$u))
  $g.FillEllipse($w2, (82*$u), (200*$u), (12*$u), (16*$u))
  $g.FillEllipse($w2, (162*$u), (200*$u), (12*$u), (16*$u))

  # ---------- 4) 星芒（四角星，两个角度） ----------
  $sb = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(205, 255, 255, 255))
  foreach ($s in @(@(192, 194, 15), @(54, 182, 10))) {
    $cx = $s[0]*$u; $cy = $s[1]*$u; $rr = $s[2]*$u
    $t = 0.28 * $rr
    foreach ($rot in @(0, 45)) {
      $st = $g.Transform.Clone()
      $g.TranslateTransform($cx, $cy); $g.RotateTransform($rot)
      $pts = @(
        [System.Drawing.PointF]::new(0, -$rr),
        [System.Drawing.PointF]::new($t, -$t),
        [System.Drawing.PointF]::new($rr, 0),
        [System.Drawing.PointF]::new($t, $t),
        [System.Drawing.PointF]::new(0, $rr),
        [System.Drawing.PointF]::new(-$t, $t),
        [System.Drawing.PointF]::new(-$rr, 0),
        [System.Drawing.PointF]::new(-$t, -$t)
      )
      $g.FillPolygon($sb, $pts)
      $g.Transform = $st; $st.Dispose()
    }
  }
  $sb.Dispose()

  $g.Dispose()
  return $bmp
}

# ---------- 组装 ICO（多尺寸 PNG-in-ICO） ----------
$sizes = @(16, 24, 32, 48, 64, 128, 256)
$images = @()
foreach ($sz in $sizes) {
  $bmp = New-IconBitmap $sz
  $ms = New-Object System.IO.MemoryStream
  $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
  $images += ,@($ms.ToArray(), $sz, $sz)
  if ($sz -eq 256) { $bmp.Save($outPng, [System.Drawing.Imaging.ImageFormat]::Png) }
  $bmp.Dispose(); $ms.Dispose()
}

$os = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter($os)
$bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$images.Count)
$off = 6 + 16 * $images.Count
foreach ($img in $images) {
  $n = if ($img[1] -ge 256) { 0 } else { $img[1] }
  $m = if ($img[2] -ge 256) { 0 } else { $img[2] }
  $bw.Write([byte]$n); $bw.Write([byte]$m); $bw.Write([byte]0); $bw.Write([byte]0)
  $bw.Write([uint16]1); $bw.Write([uint16]32)
  $bw.Write([uint32]$img[0].Length); $bw.Write([uint32]$off)
  $off += $img[0].Length
}
foreach ($img in $images) { $bw.Write($img[0]) }
$bw.Flush()
[System.IO.File]::WriteAllBytes($outIco, $os.ToArray())
$bw.Dispose(); $os.Dispose()

Write-Output ("OK: {0} ({1} bytes) preview: {2}" -f $outIco, (Get-Item $outIco).Length, $outPng)
