param([Parameter(Mandatory=$true)][string]$JobDirectory)
$ErrorActionPreference='Stop'
try {
 Add-Type -AssemblyName System.Windows.Forms
 Add-Type -AssemblyName System.Drawing
 Add-Type -AssemblyName System.Management
 Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing,System.Management,System.Web.Extensions -TypeDefinition @'
using System;
using System.IO;
using System.Drawing;
using System.Drawing.Printing;
using System.Windows.Forms;
using System.Management;
using System.Web.Script.Serialization;
using System.Collections.Generic;
public class OrigoPrint {
 public class Page {public string file;public float width;public float height;public int label;}
 public class Job {public Page[] pages;}
 public static void Run(string directory) {
  Exception failure=null;
  var ui=new System.Threading.Thread(()=>{try{RunUi(directory);}catch(Exception error){failure=error;}});
  ui.SetApartmentState(System.Threading.ApartmentState.STA);ui.Start();ui.Join();
  if(failure!=null)throw failure;
 }
 private static void RunUi(string directory) {
  var serializer=new JavaScriptSerializer();serializer.MaxJsonLength=1048576;
  var job=serializer.Deserialize<Job>(File.ReadAllText(Path.Combine(directory,"job.json")));
  if(job==null||job.pages==null||job.pages.Length==0||job.pages.Length>1024)throw new Exception("Invalid print manifest");
  foreach(var p in job.pages)if(Path.GetFileName(p.file)!=p.file||!p.file.EndsWith(".png")||p.width<=0||p.height<=0)throw new Exception("Invalid print page");
  Application.EnableVisualStyles();
  using(var document=new PrintDocument())using(var form=new Form())using(var picture=new PictureBox())using(var timer=new Timer()) {
   document.DocumentName="Origo";document.PrintController=new StandardPrintController();
   var iconPath=Environment.GetEnvironmentVariable("ORIGO_PRINT_EXECUTABLE");if(!String.IsNullOrEmpty(iconPath))form.Icon=Icon.ExtractAssociatedIcon(iconPath);
   form.MinimumSize=new Size(600,400);form.Text="Cetak - Origo";form.Width=820;form.Height=760;form.StartPosition=FormStartPosition.CenterScreen;
   var bar=new FlowLayoutPanel {Dock=DockStyle.Top,Height=95,AutoSize=false,WrapContents=true};
   var printer=new Button {Text="Printer...",AutoSize=true};var setup=new Button {Text="Kertas / orientasi...",AutoSize=true};
   var fit=new CheckBox {Text="Fit ke area cetak",Checked=true,AutoSize=true};
   var previous=new Button {Text="Sebelumnya",AutoSize=true};var next=new Button {Text="Berikutnya",AutoSize=true};
   var print=new Button {Text="Cetak",AutoSize=true};var cancel=new Button {Text="Batal",AutoSize=true};
   var info=new Label {AutoSize=true};var explanation=new Label {AutoSize=true,Text="Anotasi dan tanda tangan disertakan. Ukuran asli = 100%. Cetak ke PDF menghasilkan halaman gambar."};
   bar.Controls.AddRange(new Control[]{printer,setup,fit,previous,next,print,cancel,info,explanation});
   picture.Dock=DockStyle.Fill;picture.BackColor=Color.DimGray;
   form.Controls.Add(picture);form.Controls.Add(bar);
   int index=0,printIndex=0;bool submitted=false;bool fileOutput=false;bool choosing=false;
   Func<bool> cancelled=()=>File.Exists(Path.Combine(directory,"cancel"));
   Action update=()=>{info.Text=String.Format("Halaman {0} ({1}/{2}) - {3}",job.pages[index].label,index+1,job.pages.Length,document.PrinterSettings.PrinterName);picture.Invalidate();};
   Func<bool> choose=()=>{if(choosing)return false;choosing=true;try {using(var dialog=new PrintDialog {Document=document,UseEXDialog=false,AllowSomePages=false,AllowPrintToFile=false,ShowHelp=false}) {
     if(dialog.ShowDialog(form)!=DialogResult.OK)return false;
     document.PrinterSettings.PrintToFile=false;update();return true;
   }}finally{choosing=false;}};
   Action<Graphics,RectangleF,int> draw=(graphics,area,which)=>{
    var page=job.pages[which];using(var image=Image.FromFile(Path.Combine(directory,page.file))) {
     float width=page.width*100f/72f,height=page.height*100f/72f;
     float scale=fit.Checked?Math.Min(area.Width/width,area.Height/height):1f;
     width*=scale;height*=scale;
     var target=new RectangleF(area.X+(area.Width-width)/2,area.Y+(area.Height-height)/2,width,height);
     graphics.DrawImage(image,target);
    }
   };
   picture.Paint+=(sender,e)=>{
    try {
     if(!document.PrinterSettings.IsValid)return;
     var settings=document.DefaultPageSettings;var paper=settings.Bounds;var printable=settings.PrintableArea;
     float scale=Math.Min((picture.Width-32f)/paper.Width,(picture.Height-32f)/paper.Height);
     e.Graphics.TranslateTransform((picture.Width-paper.Width*scale)/2,(picture.Height-paper.Height*scale)/2);
     e.Graphics.ScaleTransform(scale,scale);e.Graphics.FillRectangle(Brushes.White,new RectangleF(0,0,paper.Width,paper.Height));
     var saved=e.Graphics.Save();e.Graphics.SetClip(printable);draw(e.Graphics,printable,index);e.Graphics.Restore(saved);
     e.Graphics.DrawRectangle(Pens.Gray,printable.X,printable.Y,printable.Width,printable.Height);
    }catch(Exception error){info.Text=error.Message;}
   };
   printer.Click+=(s,e)=>{choose();update();};
   setup.Click+=(s,e)=>{using(var dialog=new PageSetupDialog {Document=document,AllowMargins=false})dialog.ShowDialog(form);update();};
   fit.CheckedChanged+=(s,e)=>update();picture.Resize+=(s,e)=>picture.Invalidate();
   previous.Click+=(s,e)=>{index=Math.Max(0,index-1);update();};next.Click+=(s,e)=>{index=Math.Min(job.pages.Length-1,index+1);update();};
   cancel.Click+=(s,e)=>form.Close();
   document.BeginPrint+=(s,e)=>{printIndex=0;if(cancelled())e.Cancel=true;};
   document.PrintPage+=(s,e)=>{
    if(cancelled()){e.Cancel=true;return;}
    var area=e.PageSettings.PrintableArea;
    // PrintDocument's default origin is the printable area's top-left.
    draw(e.Graphics,new RectangleF(0,0,area.Width,area.Height),printIndex);
    printIndex++;e.HasMorePages=printIndex<job.pages.Length;
   };
   print.Click+=(s,e)=>{
    if(choosing||submitted)return;
    try {
     if(document.PrinterSettings.Copies>99)throw new Exception("Maksimal 99 salinan per job.");
     if(!document.PrinterSettings.IsValid)throw new Exception("Printer tidak tersedia.");
     string name=document.PrinterSettings.PrinterName,port="",driver="";
     using(var search=new ManagementObjectSearcher("SELECT Name,PortName,DriverName FROM Win32_Printer"))using(var entries=search.Get()) {
      foreach(ManagementObject item in entries)using(item){if(String.Equals(Convert.ToString(item["Name"]),name,StringComparison.OrdinalIgnoreCase)){port=Convert.ToString(item["PortName"]);driver=Convert.ToString(item["DriverName"]);break;}}
     }
     fileOutput=String.Equals(driver,"Microsoft Print To PDF",StringComparison.OrdinalIgnoreCase);
     string upper=port.ToUpperInvariant();
     bool hardware=System.Text.RegularExpressions.Regex.IsMatch(upper,@"^(USB[0-9]+|LPT[0-9]+:?|COM[0-9]+:?)$")||upper.StartsWith("WSD-");
     if(!fileOutput&&!hardware) {
      using(var search=new ManagementObjectSearcher("SELECT Name FROM Win32_TCPIPPrinterPort"))using(var entries=search.Get()) {
       foreach(ManagementObject item in entries)using(item){if(String.Equals(Convert.ToString(item["Name"]),port,StringComparison.OrdinalIgnoreCase)){hardware=true;break;}}
      }
     }
     // Unknown/custom and file ports could open a driver save dialog that bypasses
     // Origo's source/no-overwrite checks, so they require a future reviewed adapter.
     if(!fileOutput&&!hardware)
       throw new Exception("Port printer ini belum didukung. Gunakan USB/LPT/COM/WSD/TCP-IP atau Microsoft Print to PDF.");
     if(fileOutput){document.PrinterSettings.PrintToFile=true;document.PrinterSettings.PrintFileName=Path.Combine(directory,"output.pdf");document.PrinterSettings.Copies=1;}
     print.Enabled=false;document.Print();
     if(cancelled())throw new Exception("Cetak dibatalkan; sebagian halaman mungkin sudah masuk antrean.");
     if(fileOutput&&!File.Exists(Path.Combine(directory,"output.pdf")))throw new Exception("Printer PDF tidak menghasilkan file sementara.");
     submitted=true;form.Close();
    }catch(Exception error){print.Enabled=true;MessageBox.Show(form,error.Message,"Cetak belum selesai",MessageBoxButtons.OK,MessageBoxIcon.Error);}
   };
   timer.Interval=200;timer.Tick+=(s,e)=>{if(cancelled())form.Close();};timer.Start();
   form.Shown+=(s,e)=>{if(!document.PrinterSettings.IsValid&&!choose()){form.Close();return;}update();};
   // Keep the native message loop on its own STA, independent of the host runspace.
   Application.Run(form);
   File.WriteAllText(Path.Combine(directory,"result.json"),serializer.Serialize(new {status=submitted?(fileOutput?"pdf":"printed"):"cancelled"}));
  }
 }
}
'@
 [OrigoPrint]::Run($JobDirectory)
}catch{
 [IO.File]::WriteAllText((Join-Path $JobDirectory 'result.json'),(@{status='error';message=$_.Exception.Message}|ConvertTo-Json -Compress))
 exit 1
}
