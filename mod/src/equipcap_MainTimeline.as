package _02_010_EquipTop_fla
{
   import flash.display.GradientType;
   import flash.display.MovieClip;
   import flash.display.Shape;
   import flash.geom.Matrix;

   // menu/02_010_equipcap.gfx: the vanilla part list window (02_010_EquipTop) as the inventory cap opens it (VENTE when
   // the inventory is over the cap). The 3D AC view (PlayerModelView) and the AC specs slot (DetailStatusView) are
   // removed from the timeline (tools/build_equipcap.py); this class draws the dialog panel under the rest (the vanilla dialogs' blue, framed like the tutorial window; the rest of the screen stays visible):
   // part categories, the list, the part's description and its stats only.
   [SWF(width="1920", height="1080", backgroundColor="#787878", frameRate="30")]
   public dynamic class MainTimeline extends MovieClip
   {

      public var MenuTitle:MovieClip;

      public var StatusBar:MovieClip;

      public var ItemList:MovieClip;

      public var AdditionalProperties:MovieClip;

      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1,8,this.frame9,16,this.frame17,24,this.frame25,31,this.frame32);
         this.addChildAt(this.panel(), 0);
      }

      // panel bounds (stage pixels): around the title, the list and the part panels, centred (the vanilla x 60..1220
      // moved right by 320 in this movie and in our copies of 02_Equip and 02_GaitemStatus_*: build_capmovies.py)
      private static const PX:Number = 380;
      private static const PY:Number = 20;
      private static const PW:Number = 1160;
      private static const PH:Number = 980;
      private static const CORNER:Number = 14;

      private function panel() : Shape
      {
         var s:Shape = new Shape();
         var g:* = s.graphics;
         var m:Matrix = new Matrix();
         m.createGradientBox(PW, PH, Math.PI / 2, PX, PY);
         g.beginGradientFill(GradientType.LINEAR, [0x3F506B, 0x2D3B54], [1, 1], [0, 255], m);
         g.drawRect(PX, PY, PW, PH);
         g.endFill();
         g.lineStyle(1, 0x8A9AB4, 0.5);
         g.drawRect(PX, PY, PW, PH);
         g.lineStyle(2, 0xDDE3EC, 0.9);
         for each (var c:Array in [[PX, PY, 1, 1], [PX + PW, PY, -1, 1], [PX, PY + PH, 1, -1], [PX + PW, PY + PH, -1, -1]])
         {
            g.moveTo(c[0] + c[2] * CORNER, c[1]); g.lineTo(c[0], c[1]); g.lineTo(c[0], c[1] + c[3] * CORNER);
         }
         return s;
      }

      internal function frame1() : *
      {
         stop();
      }

      internal function frame9() : *
      {
         gotoAndPlay("Loop");
      }

      internal function frame17() : *
      {
         stop();
      }

      internal function frame25() : *
      {
         gotoAndPlay("Loop");
      }

      internal function frame32() : *
      {
         stop();
      }
   }
}
