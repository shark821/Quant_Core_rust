use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use eframe::egui::{self, text, viewport};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use rayon::prelude::*;
fn main()->eframe::Result<()> {
    let option = eframe::NativeOptions{
        viewport:egui::ViewportBuilder::default().with_inner_size([1000.0,600.0]),
        ..Default::default()
    };

    eframe::run_native("QuantDash", option, Box::new(|_cc| Box::new(Quantcore::new(200,100,"2330".to_string()))))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Queto{       //存放個股歷史行情
    pub symbol: String,
    pub date: Vec<usize>,
    pub open_price: Vec<f64>,
    pub max_price: Vec<f64>,
    pub min_price: Vec<f64>,
    pub close_price: Vec<f64>,
    pub volume:Vec<f64>,
}

#[derive(Clone,Debug,Default)]
pub struct sign_data{       //存放個股指標數據
    pub rsi_data: Vec<f64>,

}

impl sign_data {
    fn new()->sign_data{
        Self{rsi_data:Vec::with_capacity(0)}
    }
}

#[derive(Clone,Debug,Default)]
struct Quantcore{       //系統內核，存放各項資料與策略開關
    data:Queto,
    rsi_use:bool,
    rsi_period:usize,
    rsi_up: f64,
    rsi_low: f64,
    rsi_trade_percent: f64,
    rsi_roi: f64,
    golden_cross:bool,
    sign_data:sign_data,
    load_days:usize,
    invest_days:usize,
}

impl Quantcore {
    fn new(his_days:usize,invest_days:usize,stock_symbol:String)->Self{
        Self{
            data:Queto{
                    symbol: stock_symbol,
                    date:Vec::with_capacity(his_days),
                    open_price:Vec::with_capacity(his_days),
                    max_price:Vec::with_capacity(his_days),
                    min_price:Vec::with_capacity(his_days),    
                    close_price:Vec::with_capacity(his_days),
                    volume:Vec::with_capacity(his_days),
            },

            rsi_use:false,
            rsi_period:14,
            rsi_up: 70.0,
            rsi_low: 30.0,
            rsi_trade_percent: 100.0,
            rsi_roi:0.0,
            golden_cross:false,
            sign_data:sign_data::new(),
            load_days:his_days,
            invest_days:invest_days,

            }
    }

    fn load_data(&mut self){
        self.data.date.clear();
        self.data.open_price.clear();
        self.data.max_price.clear();
        self.data.min_price.clear();
        self.data.close_price.clear();
        self.data.volume.clear();

        let dir_path = "stock data";

        if let Err(e) = fs::create_dir_all(dir_path) {
            println!("creat dir fail: {}", e);
            return;
        }

        let bin_path = format!("{}/{}.bin",dir_path,self.data.symbol);     //  資料下載
        if !Path::new(&bin_path).exists(){
            println!("Downloading data for {}...", self.data.symbol);
            let url = format!("https://api.finmindtrade.com/api/v4/data?dataset=TaiwanStockPrice&data_id={}&start_date=2010-01-01", self.data.symbol);      //FinMind網址
            match ureq::get(&url).call() {
                Ok(resp)=>{
                    let text = resp.into_string().unwrap_or_default();

                    //json解析器

                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(data_array) =  json["data"].as_array(){
                            for item in data_array{     //item是每一天的資料
                                let date_str = item["date"].as_str().unwrap_or("");     //載入日期，錯誤就空着
                                let date_int = date_str.replace("-","").parse::<usize>().unwrap_or(0);        //轉成整數

                                let open = item["open"].as_f64().unwrap_or(0.0);        //讀取並轉型別，如果沒值就填0.0
                                let close = item["close"].as_f64().unwrap_or(0.0);
                                let max = item["max"].as_f64().unwrap_or(0.0);
                                let min = item["min"].as_f64().unwrap_or(0.0);
                                let vol = item["Trading_Volume"].as_f64().unwrap_or(0.0);

                                self.data.date.push(date_int);      //塞入資料向量
                                self.data.open_price.push(open);
                                self.data.close_price.push(close);
                                self.data.max_price.push(max);
                                self.data.min_price.push(min);
                                self.data.volume.push(vol);

                            }
                            
                        }
                        
                    }
                    //json轉bin
                    if let Ok(encoded) = bincode::serialize(&self.data) {
                        if let Err(e) = fs::write(&bin_path, encoded) {
                            println!("bin save fail: {}", e);
                        } else {
                            println!("bin save sucess！");
                        }
                    }
                }

                Err(e) => {
                    println!("Download failed: {}", e);();
                    return;
                }
            }
        }

        else {
            println!("Load from local cache: {}", bin_path);

            if let Ok(bytes) = fs::read(&bin_path) {
                if let Ok(cached_data) = bincode::deserialize::<Queto>(&bytes) {
                    self.data = cached_data; // 記憶體覆蓋
                    println!("bin load sucess！len: {}", self.data.close_price.len());
                } else {
                    println!("bin load fail");
                }
            }
        }
        }
    
    fn sign_compute(&mut self,rsi_button:bool) {
        
        self.rsi_use=rsi_button;
        let mut rsi_data=Vec::with_capacity(self.invest_days);
        self.golden_cross=false;
        let len:usize = self.data.date.len();

        if len>self.invest_days+self.rsi_period{
            let start_index=len-self.invest_days;
            if start_index>=self.rsi_period{
                rsi_data = (start_index..len).into_iter().map(|i| self.rsi(i,self.rsi_period)).collect();       //使用迭代器取代for迴圈以加速計算
                
            }
            else{
                println!("rsi_period + invest days > history days, You need to increase history which before invest")
            }
            
        }

        else {
            println!("rse_period > data days");
        }

        self.sign_data=sign_data { rsi_data:  rsi_data };
    }
    fn rsi(&self,i:usize,rsi_period:usize)->f64{
        if i<rsi_period{println!("rsi_period > history days before invest day, You need to increase history which before invest");}
        let price = &self.data.close_price;
        let mut sum_gain:f64 = 0.0;
        let mut sum_loss:f64 = 0.0;

        if i < rsi_period { 
            return f64::NAN; 
        }

        for j in 0..rsi_period{
            let diff = price[i-j]-price[i-j-1];
            if diff>0.0 {
                sum_gain+=diff;
            }
            else {
                sum_loss+=diff.abs();
            }
        }

        let avg_gain:f64=sum_gain/rsi_period as f64;
        let avg_loss:f64 =sum_loss/rsi_period as f64;

        let rs:f64=avg_gain/avg_loss;

        if rs==0.0{return 100.0;}

        100.0-(100.0/(1.0+rs))      //最終rsi指標
        
    }

    fn rsi_backtest(&self,up:f64,low:f64,trade_percent:f64,cash:f64)->f64 {
        let rsi_data=&self.sign_data.rsi_data;
        if rsi_data.is_empty() { return 1.0; } // 防呆：沒資料就回傳本金
        let mut cash = cash;
        let mut stocks = 0.0;

        // 對齊股價陣列的起始點
        let start_index = self.data.close_price.len() - self.invest_days;
        
        for i in 1..self.sign_data.rsi_data.len(){
            let prev_rsi = rsi_data[i - 1];
            let curr_rsi = rsi_data[i];

            if prev_rsi.is_nan() || curr_rsi.is_nan() { continue; }     //處理nan

            let current_price = self.data.close_price[start_index + i];     //當日收盤

            if curr_rsi>up && prev_rsi<=up{
                //sell
                let  sell_stocks = (stocks*(trade_percent/100.0)).floor();
                if sell_stocks <1.0 && stocks >=1.0{
                    cash+=current_price;
                    stocks+=-1.0;
                }
                else if sell_stocks >1.0 && stocks >=sell_stocks {
                    cash+=sell_stocks*current_price;
                    stocks+=(-1.0)*sell_stocks
                }
                else if sell_stocks >1.0 && stocks < sell_stocks && stocks >0.0 {
                    cash+=stocks*current_price;
                    stocks=0.0
                }
            }
            else if rsi_data[i]<low && rsi_data[i-1]>=low{
                //buy
                let  buy_stocks = (cash*(trade_percent/100.0)/current_price).floor();
                if cash < current_price{
                } 
                else if cash>= current_price && buy_stocks < 1.0 {
                    cash+=(-1.0)*current_price;
                    stocks+=1.0;
                }
                else if buy_stocks >=1.0 {
                    cash+=(-1.0)*buy_stocks*current_price;
                    stocks+=buy_stocks;
                }
            } 
        }

        let last_price = self.data.close_price.last().unwrap_or(&0.0);
        return cash + stocks * last_price;

    }

    fn rsi_optimizer(&self,init_capital:usize) -> Option<(f64, f64, f64, f64)>{
        let rsi_data=&self.sign_data.rsi_data;
        //產生參數組合網格
        let mut param_grid = Vec::new();
        for l in 15..=40 {
            for u in 60..=85 {
                for p in 10..=90{
                    param_grid.push((u as f64, l as f64,p as f64));
                }
            }
        }

        let best_result=param_grid.into_par_iter().map(|(up_line,low_line,trade_percent)|{
            let init_capital = init_capital as f64;
            let final_capital = self.rsi_backtest(up_line, low_line, trade_percent, init_capital);
            let ROI = final_capital/init_capital*100.0-100.0;
            (ROI,up_line,low_line,trade_percent)

        }).max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        best_result
    }
    }

impl eframe::App for Quantcore {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {      //渲染
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("QuantDash");        //標題
            ui.horizontal(
                    |ui|{       //元件水平並排
                        ui.label("Stock_symbol");       //文字格說明
                        ui.text_edit_singleline(&mut self.data.symbol);      //文字框

                        ui.label("history_days");
                        ui.add(egui::DragValue::new(&mut self.load_days));      //數字框 歷史載入天數
                        
                        ui.label("invest_days");
                        if ui.add(egui::DragValue::new(&mut self.invest_days)).changed(){//數字框 實際投資天數
                            if self.rsi_use {
                            self.sign_compute(true);
                            }
                        }

                        let response = ui.checkbox(&mut self.rsi_use, "Open RSI");      //RSI開關

                        if response.changed(){
                            if self.rsi_use{
                                self.sign_compute(true);
                            }
                            else {
                                self.sign_compute(false);
                            }
                        }

                        if ui.button("Reload Data").clicked(){      //重載按鈕
                            self.load_data();

                            if self.rsi_use {
                                self.sign_compute(true);
                            }
                             
                            println!("sucess load {} days data for {} stock",self.load_days,self.data.symbol);
                        }

                        
                    }
            );
            ui.separator();//分隔線

            let show_text=format!("ROI: {:.2}%, Buy line: {}, Sell line: {}, Trade percentage: {}%", self.rsi_roi,self.rsi_low, self.rsi_up, self.rsi_trade_percent);
            ui.label(show_text);

            if ui.button("Optimize RSI").clicked() {
                if let Some((ROI, best_up, best_low, best_p)) = self.rsi_optimizer(10000) {
                    println!("🏆 最佳化完成！");
                    println!("報酬率: {:.2}%, 買線: {}, 賣線: {}, 交易百分比: {}%", ROI, best_low, best_up, best_p);
                    //參數更新
                    self.rsi_up = best_up;      
                    self.rsi_low = best_low;
                    self.rsi_trade_percent = best_p;    
                    self.rsi_roi=ROI;   
                    if self.rsi_use { self.sign_compute(true); }        // 重新計算，更新畫面
                }
            };

            ui.separator();//分隔線
            ui.label("Close Price");

            // 如果 RSI 開啟，就把主圖比例壓扁一點讓出空間
            let main_aspect = if self.rsi_use { 4.0 } else { 2.5 };
            let plot = Plot::new("market_plot").view_aspect(main_aspect);

            plot.show(ui, |plot_ui|{        //畫折線圖
                let len = self.data.close_price.len();      //圖表大小

                if len==0 {return;}

                let start_index = if len > self.load_days { len - self.load_days } else { 0 };
                let mut plot_points = Vec::with_capacity(len - start_index);

                
                for i in start_index..len{      //只畫切片部分
                    let x= -((len - i) as f64);
                    let y =self.data.close_price[i] ;
                    plot_points.push([x,y]);
                }

                plot_ui.line(Line::new(plot_points).name("Close Price"));

                if self.invest_days > 0 {
                    // 計算投資起始日座標 (例如 invest_days=100，則 x = -99.0)
                    let invest_start_x = -((self.invest_days) as f64);
                    
                    plot_ui.vline(
                        egui_plot::VLine::new(invest_start_x)
                            .name("invest day")
                            .color(egui::Color32::LIGHT_BLUE) 
                            .style(egui_plot::LineStyle::Dashed { length: 5.0 }) // 虛線
                    );
                }

            });

            if self.rsi_use{
                ui.separator();
                ui.label("RSI Curve");
                let plot_rsi = Plot::new("RSI plot").view_aspect(6.0);
                plot_rsi.show(ui,|plot_ui|{
                    let rsi_len = self.sign_data.rsi_data.len();
                    if rsi_len == 0 { return; }

                    let mut plot_points = Vec::with_capacity(rsi_len);
                
                    for i in 0..rsi_len{      //只畫切片部分
                        let x= -((rsi_len - i) as f64);
                        let y =self.sign_data.rsi_data[i] ;
                        if !y.is_nan() {
                            plot_points.push([x,y]);
                        }
                    }

                    plot_ui.line(Line::new(plot_points).name("RSI Value"));
                    plot_ui.hline(
                        egui_plot::HLine::new(self.rsi_up)
                            .name("Sell Line")
                            .color(egui::Color32::RED)
                    );

                    // 畫出超賣線 (綠色)
                    plot_ui.hline(
                        egui_plot::HLine::new(self.rsi_low)
                            .name("Buy Line")
                            .color(egui::Color32::GREEN)
                    );
                    
                    }
                );
            }
      
                });
        
        
    }
    
}