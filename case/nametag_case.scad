// Velcro-backed case for the LilyGO T5 4.7" e-paper S3 (V2.3).
// Two parts: tray (back, velcro side) + bezel (front, display window).
// Stack inside, front to back: clear lens -> panel+PCB -> LiPo pouch.
// Board numbers come from LilyGO's EPD47-S3.dxf; MEASURE YOUR BOARD and tweak.
//
// Render:  openscad -D 'part="tray"'  -o tray.stl  nametag_case.scad
//          openscad -D 'part="bezel"' -o bezel.stl nametag_case.scad

part = "both";          // "tray" | "bezel" | "both"

// --- board (from DXF: PCB 118.1 x 63.1, panel glass 0.87 thick) ---
pcb_w = 118.1;
pcb_h = 63.1;
fit   = 0.25;           // clearance per side, raise if your printer runs fat
front_stack = 2.5;      // panel + PCB thickness, front of glass to PCB back
back_parts  = 4.5;      // tallest part on the PCB back (USB-C, buttons, JST)
lens_t = 1.0;           // clear acrylic/PETG protector over the glass; 0 = none
bat_t  = 5.0;           // LiPo thickness (503450 = 5 mm)

// --- shell ---
wall  = 4;              // thick enough for M2 corner screws
floor_t = 2;
bezel_t = 2;
win_w = 105;            // active area is 103.68 x 58.32
win_h = 59.5;
win_dx = 0;             // nudge window if your panel isn't centred
win_dy = 0;

// --- side cutouts (USB-C on a short end, RST/IO0/IO21 on a long edge) ---
mirror_ports = false;   // flip if the cutouts land on the wrong side
usb_y  = 7.8;           // USB-C centre offset from board centre line
usb_w  = 12; usb_hgt = 7.5;
btn_from_usb_end = 27;  // RST/BOOT/IO21 sit 28.5..50.9 mm from the USB end
btn_len = 25; btn_hgt = 6;

// --- velcro: recess on the back for a strip of adhesive hook tape ---
velcro_w = 100; velcro_h = 50; velcro_d = 0.8;
stitch_d = 1.6;         // holes in recess corners to sew/zip-tie the strip

screw_pilot = 1.6;      // M2 self-tapping into plastic
screw_clear = 2.4;

$fn = 48;

in_w = pcb_w + 2*fit;
in_h = pcb_h + 2*fit;
depth = lens_t + front_stack + back_parts + bat_t;
tray_h = floor_t + depth;
pcb_back_z = tray_h - lens_t - front_stack;
port_z = pcb_back_z - back_parts/2;
screws = [for (sx=[-1,1], sy=[-1,1]) [sx*(in_w/2+wall/2), sy*(in_h/2+wall/2)]];
s = mirror_ports ? -1 : 1;

module outline() {
    offset(r=wall/2) square([in_w+wall, in_h+wall], center=true);
}

module tray() {
    difference() {
        linear_extrude(tray_h) outline();
        translate([0,0,floor_t]) linear_extrude(depth+1) square([in_w, in_h], center=true);
        // corner pilot holes
        for (p=screws) translate([p[0],p[1],tray_h-10]) cylinder(d=screw_pilot, h=11);
        // USB-C (short end, -X)
        translate([-in_w/2-wall-1, s*usb_y-usb_w/2, port_z-usb_hgt/2])
            cube([wall+2, usb_w, usb_hgt]);
        // button slot (long edge, -Y)
        translate([-in_w/2+btn_from_usb_end, -s*(in_h/2+wall/2)-(wall+2)/2, port_z-btn_hgt/2])
            cube([btn_len, wall+2, btn_hgt]);
        // velcro recess + stitch holes
        translate([0,0,-1]) linear_extrude(velcro_d+1) square([velcro_w, velcro_h], center=true);
        for (sx=[-1,1], sy=[-1,1])
            translate([sx*(velcro_w/2-4), sy*(velcro_h/2-4), -1]) cylinder(d=stitch_d, h=floor_t+2);
    }
}

module bezel() {
    difference() {
        linear_extrude(bezel_t) outline();
        translate([win_dx, win_dy, -1]) linear_extrude(bezel_t+2)
            offset(r=1) square([win_w-2, win_h-2], center=true);
        for (p=screws) translate([p[0],p[1],-1]) cylinder(d=screw_clear, h=bezel_t+2);
    }
}

if (part == "tray") tray();
if (part == "bezel") bezel();   // print face-down
if (part == "both") { tray(); translate([0,0,tray_h+10]) bezel(); }
